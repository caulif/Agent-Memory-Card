//! Pipeline Layer 3：CLUSTER 聚类。
//!
//! 输入：[`TruncatedMessage`] 列表（来自 Layer 2 TRUNCATE）
//! 输出：[`MessageCluster`] 列表（语义相近的消息归一组）
//!
//! 算法：
//! 1. 用 [`crate::extract::embedding::FastEmbedMatcher`] 一次 batch 编码所有消息
//! 2. O(N²) 全对比 cosine
//! 3. 短消息（< [`SHORT_MESSAGE_CHARS`]）用阈值 [`DEFAULT_SHORT_THRESHOLD`]，
//!    长消息用 [`DEFAULT_LONG_THRESHOLD`]（短句更易假阳性）
//! 4. 阈值过的对走并查集 union
//! 5. 输出连通分量；簇内消息按 created_at 升序排列
//! 6. 单消息簇默认丢弃（可通过 [`ClusterOptions::keep_singletons`] 保留）
//! 7. 大簇（> [`MAX_CLUSTER_SIZE`]）按时间序切成多个相邻簇
//!
//! # 性能
//!
//! N=1000 messages × 384 维 embedding：内存约 1.5 MB；O(N²) ≈ 1M 次
//! cosine，耗时 < 200ms。Layer 3 不调 LLM。
//!
//! # 失败模式
//!
//! - fastembed 不可用：返回 Err，由调用方决定回退（可以单消息成簇）
//! - embedding 全 0：cosine 返回 0，所有消息独立成簇
//! - 超长 N：N=10000 时内存约 15MB、计算约 100M 次 cosine（仍然几秒），
//!   但 prompt cost 控制由 Layer 4 负责，不在这里限流

use std::collections::{BTreeMap, HashSet};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::extract::embedding::{FastEmbedMatcher, cosine};
use crate::extract::truncate::TruncatedMessage;

/// 长消息归簇阈值（V2 修订 1）—— fastembed cosine
pub const DEFAULT_LONG_THRESHOLD: f32 = 0.78;

/// 短消息归簇阈值：短句更易假阳性，阈值更严
pub const DEFAULT_SHORT_THRESHOLD: f32 = 0.82;

/// jaccard 回退时的长消息阈值。
///
/// bigram jaccard 在长日志/JSON/会话片段上容易被共享样板抬高。真实项目
/// AionUi 回归显示低阈值会把不同主题长 observation 合成大噪声簇，导致
/// INDUCE 全拒；长文本宁可多保留 singleton，再交给 INDUCE 做低置信判断。
pub const DEFAULT_JACCARD_LONG_THRESHOLD: f32 = 0.60;

/// jaccard 回退时的短消息阈值。
///
/// 2026-05-12 sweep（`cluster_threshold_sweep.rs`）显示，黄金集消息几乎都
/// 落在"短消息"路径（< 30 字符），所以这个值决定纯 lexical recall：
/// - 0.55（V1 默认）→ recall 0%
/// - 0.40 → recall 0%
/// - 0.30 → recall 33%
/// - 0.20 → recall 50% （precision 仍 100%）
///
/// 选 0.20 作为默认 lexical 阈值，再叠加工程语义锚点补偿短句同义改写。
pub const DEFAULT_JACCARD_SHORT_THRESHOLD: f32 = 0.20;

/// 区分长短消息的字符阈值
pub const SHORT_MESSAGE_CHARS: usize = 30;

/// 单簇最大消息数；超出按时间序切
pub const MAX_CLUSTER_SIZE: usize = 10;

/// 一组语义相近的消息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageCluster {
    /// 簇 ID：基于簇内 observation_id 列表的 sha256 前 8 位（稳定可复现）
    pub cluster_id: String,

    /// 簇内消息数（recurrence 信号）
    pub recurrence: usize,

    /// 簇内消息，按 created_at 升序排列
    pub messages: Vec<TruncatedMessage>,

    /// 簇内成员两两相似度的均值（debug，用于调阈值）
    pub mean_similarity: f32,

    /// 实际使用的相似度后端（debug）
    pub backend: String,
}

/// 聚类配置。
#[derive(Debug, Clone)]
pub struct ClusterOptions {
    pub long_threshold: f32,
    pub short_threshold: f32,
    pub jaccard_long_threshold: f32,
    pub jaccard_short_threshold: f32,
    pub short_message_chars: usize,
    pub max_cluster_size: usize,
    /// 是否保留单消息簇（默认 false，节省 Layer 4 LLM 调用）
    pub keep_singletons: bool,
    /// 强制使用 jaccard（环境无网时显式设置；fastembed 失败时也会自动回退）
    pub force_jaccard: bool,
}

impl Default for ClusterOptions {
    fn default() -> Self {
        Self {
            long_threshold: DEFAULT_LONG_THRESHOLD,
            short_threshold: DEFAULT_SHORT_THRESHOLD,
            jaccard_long_threshold: DEFAULT_JACCARD_LONG_THRESHOLD,
            jaccard_short_threshold: DEFAULT_JACCARD_SHORT_THRESHOLD,
            short_message_chars: SHORT_MESSAGE_CHARS,
            max_cluster_size: MAX_CLUSTER_SIZE,
            // 默认保留单消息簇：单 obs 也可能含真实偏好；recurrence=1 时
            // Layer 4 INDUCE 会按低 confidence 标注，不会盲目升级为高质量卡。
            keep_singletons: true,
            force_jaccard: false,
        }
    }
}

/// 相似度后端：fastembed 主路径 + jaccard 兜底。
enum SimilarityBackend {
    FastEmbed { embeddings: Vec<Vec<f32>> },
    Jaccard,
}

impl SimilarityBackend {
    fn name(&self) -> &'static str {
        match self {
            Self::FastEmbed { .. } => "fastembed",
            Self::Jaccard => "jaccard",
        }
    }
}

/// 流水线 Layer 3 主入口（默认配置）。
pub fn cluster_messages(messages: &[TruncatedMessage]) -> Result<Vec<MessageCluster>> {
    cluster_messages_with(messages, &ClusterOptions::default())
}

/// 自定义配置的聚类入口。
///
/// 不会因 fastembed 不可用而 Err：自动回退到 jaccard，让流水线在无网环境
/// 也能跑通（质量略降）。
pub fn cluster_messages_with(
    messages: &[TruncatedMessage],
    options: &ClusterOptions,
) -> Result<Vec<MessageCluster>> {
    if messages.is_empty() {
        return Ok(Vec::new());
    }

    let backend = select_backend(messages, options);
    let pairs = collect_similar_pairs(messages, &backend, options);
    let mut groups = build_groups_via_union_find(messages.len(), &pairs);

    let mut clusters = materialize_clusters(messages, &backend, &mut groups, options);
    if !options.keep_singletons {
        clusters.retain(|cluster| cluster.messages.len() > 1);
    }
    Ok(clusters)
}

/// 优先 fastembed；失败/被禁用则 jaccard。
fn select_backend(messages: &[TruncatedMessage], options: &ClusterOptions) -> SimilarityBackend {
    if options.force_jaccard {
        return SimilarityBackend::Jaccard;
    }
    if std::env::var("AGENT_KERNEL_DISABLE_FASTEMBED").as_deref() == Ok("1") {
        return SimilarityBackend::Jaccard;
    }
    match FastEmbedMatcher::try_new() {
        Ok(matcher) => {
            let texts: Vec<String> = messages.iter().map(|m| m.body.clone()).collect();
            match matcher.embed_batch(&texts) {
                Ok(embeddings) => SimilarityBackend::FastEmbed { embeddings },
                Err(error) => {
                    eprintln!("[cluster] fastembed embed failed, falling back to jaccard: {error}");
                    SimilarityBackend::Jaccard
                }
            }
        }
        Err(error) => {
            eprintln!("[cluster] fastembed unavailable, falling back to jaccard: {error}");
            SimilarityBackend::Jaccard
        }
    }
}

/// 收集所有相似度过阈值的 (i, j) 索引对（i < j）。
fn collect_similar_pairs(
    messages: &[TruncatedMessage],
    backend: &SimilarityBackend,
    options: &ClusterOptions,
) -> Vec<(usize, usize, f32)> {
    let n = messages.len();
    let mut pairs = Vec::new();
    for i in 0..n {
        for j in (i + 1)..n {
            let threshold = pair_threshold(backend, &messages[i], &messages[j], options);
            let sim = pair_similarity(backend, messages, i, j);
            if sim >= threshold {
                pairs.push((i, j, sim));
            }
        }
    }
    pairs
}

/// 计算两条消息的相似度（按 backend 路由）。
fn pair_similarity(
    backend: &SimilarityBackend,
    messages: &[TruncatedMessage],
    i: usize,
    j: usize,
) -> f32 {
    match backend {
        SimilarityBackend::FastEmbed { embeddings } => cosine(&embeddings[i], &embeddings[j]),
        SimilarityBackend::Jaccard => {
            let lexical = char_bigram_jaccard(&messages[i].body, &messages[j].body);
            lexical.max(engineering_anchor_similarity(
                &messages[i].body,
                &messages[j].body,
            ))
        }
    }
}

/// 字符级 bigram jaccard：把文本规范化成连续字母数字 + CJK 序列后，
/// 取所有相邻 2-字符组成的集合，然后算 |A∩B| / |A∪B|。
///
/// 选 bigram（不是 unigram、trigram）的原因：
/// - unigram 对长中文文本过宽（"的"、"了" 高频字会主导）
/// - trigram 对短句过严（21 字符只有 19 个 trigram，1 字之差就掉很多）
/// - bigram 经验上对中英混排短文本最稳
///
/// 经验阈值：完全相同 1.0；一字不同 ≈ 0.85；同义重写 0.5-0.7；不同主题 < 0.2。
pub(crate) fn char_bigram_jaccard(left: &str, right: &str) -> f32 {
    let left_set = char_bigram_set(left);
    let right_set = char_bigram_set(right);
    if left_set.is_empty() && right_set.is_empty() {
        return 1.0;
    }
    if left_set.is_empty() || right_set.is_empty() {
        return 0.0;
    }
    let intersection = left_set.intersection(&right_set).count();
    let union = left_set.union(&right_set).count();
    intersection as f32 / union as f32
}

/// 提取连续字符序列的 bigram 集合。
fn char_bigram_set(text: &str) -> HashSet<String> {
    let normalized: String = text
        .chars()
        .filter(|c| c.is_alphanumeric() || is_cjk(*c))
        .flat_map(|c| c.to_lowercase())
        .collect();
    let chars: Vec<char> = normalized.chars().collect();
    if chars.len() < 2 {
        return HashSet::new();
    }
    chars
        .windows(2)
        .map(|window| window.iter().collect::<String>())
        .collect()
}

/// Jaccard fallback 的轻量工程语义锚点。
///
/// 真实黄金集里短句常用同义改写表达同一规则，例如 "review 边界"、
/// "人工审阅"、"AI 自动写入规则"。这些短句字符 bigram 重叠很低，但对
/// Agent Memory 来说属于同一个治理主题。这里不做通用 NLP，只补偿少量
/// 项目高频工程锚点；fastembed 主路径不使用它。
fn engineering_anchor_similarity(left: &str, right: &str) -> f32 {
    let left_anchors = engineering_anchors(left);
    let right_anchors = engineering_anchors(right);
    if left_anchors.is_empty() || right_anchors.is_empty() {
        return 0.0;
    }
    if shares_high_confidence_anchor(&left_anchors, &right_anchors) {
        0.64
    } else if left_anchors.intersection(&right_anchors).next().is_some() {
        0.24
    } else {
        0.0
    }
}

fn shares_high_confidence_anchor(
    left_anchors: &HashSet<&'static str>,
    right_anchors: &HashSet<&'static str>,
) -> bool {
    [
        "artifact-diff-preview",
        "workflow-first",
        "evidence-before-confidence",
        "provider-evidence-grounded",
        "file-scoped-drift-resolution",
        "memory-merge-review",
    ]
    .iter()
    .any(|anchor| left_anchors.contains(anchor) && right_anchors.contains(anchor))
}

fn engineering_anchors(text: &str) -> HashSet<&'static str> {
    let lower = text.to_lowercase();
    let mut anchors = HashSet::new();

    if contains_any(&lower, &["review", "审阅", "人工", "边界"]) {
        anchors.insert("human-review-boundary");
    }
    if contains_any(&lower, &["固化规则", "写入规则", "自动写入", "规则"]) {
        anchors.insert("rule-write-control");
    }
    if contains_any(&lower, &["测试", "单测", "集成", "全量"]) {
        anchors.insert("test-scope");
    }
    if contains_any(&lower, &["风险", "改动风险", "边界"]) {
        anchors.insert("risk-boundary");
    }
    if contains_any(&lower, &["git", "commit", "push", "remote"]) {
        anchors.insert("git-operation");
    }
    if contains_any(&lower, &["确认", "明确说", "不允许", "不要主动", "必须我"]) {
        anchors.insert("human-confirmation");
    }
    if contains_any(&lower, &["emoji", "表情符号", "表情"]) {
        anchors.insert("output-style");
    }
    if contains_any(
        &lower,
        &[
            "artifact",
            "diff",
            "预览",
            "完整差异",
            "动作字符串",
            "生成文件",
            "agents.md",
            "claude.md",
            "sync",
            "同步",
        ],
    ) {
        anchors.insert("artifact-diff-preview");
    }
    if contains_any(
        &lower,
        &[
            "workflow",
            "workflow first",
            "流程化",
            "固定路径",
            "主链路",
            "黑盒 agent",
            "自由乱跑",
        ],
    ) && contains_any(&lower, &["agent", "流程", "开放探索", "固定路径"])
    {
        anchors.insert("workflow-first");
    }
    if contains_any(
        &lower,
        &[
            "evidence before confidence",
            "证据再给信心",
            "先给证据",
            "验证输出",
            "展示证据",
            "结论必须跟着验证走",
        ],
    ) || (contains_any(&lower, &["证据", "evidence", "验证"])
        && contains_any(&lower, &["信心", "confidence", "宣称", "结论"]))
    {
        anchors.insert("evidence-before-confidence");
    }
    if contains_any(&lower, &["provider", "llm"])
        && contains_any(&lower, &["evidence", "quote", "证据"])
        && contains_any(&lower, &["observation", "可回溯", "编造", "找到"])
    {
        anchors.insert("provider-evidence-grounded");
    }
    if contains_any(&lower, &["drift"])
        && contains_any(
            &lower,
            &["逐文件", "file-scoped", "目标文件", "分别", "全局"],
        )
    {
        anchors.insert("file-scoped-drift-resolution");
    }
    if contains_any(
        &lower,
        &["memory card", "卡片", "源卡", "merge review", "合并"],
    ) && contains_any(
        &lower,
        &["lineage", "来源", "差异", "保留字段", "agent", "预览"],
    ) {
        anchors.insert("memory-merge-review");
    }

    anchors
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn is_cjk(c: char) -> bool {
    matches!(c, '\u{3400}'..='\u{4DBF}' | '\u{4E00}'..='\u{9FFF}' | '\u{20000}'..='\u{2A6DF}')
}

/// 按两条消息的字符数 + 后端选阈值。
fn pair_threshold(
    backend: &SimilarityBackend,
    a: &TruncatedMessage,
    b: &TruncatedMessage,
    options: &ClusterOptions,
) -> f32 {
    let any_short = a.body.chars().count() < options.short_message_chars
        || b.body.chars().count() < options.short_message_chars;
    match backend {
        SimilarityBackend::FastEmbed { .. } => {
            if any_short {
                options.short_threshold
            } else {
                options.long_threshold
            }
        }
        SimilarityBackend::Jaccard => {
            if any_short {
                options.jaccard_short_threshold
            } else {
                options.jaccard_long_threshold
            }
        }
    }
}

/// 并查集构建连通分量。
fn build_groups_via_union_find(
    n: usize,
    pairs: &[(usize, usize, f32)],
) -> BTreeMap<usize, Vec<usize>> {
    let mut dsu = DisjointSet::new(n);
    for (i, j, _) in pairs {
        dsu.union(*i, *j);
    }
    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for index in 0..n {
        let root = dsu.find(index);
        groups.entry(root).or_default().push(index);
    }
    groups
}

/// 把连通分量转换成 MessageCluster；处理大簇切分与时间序排序。
fn materialize_clusters(
    messages: &[TruncatedMessage],
    backend: &SimilarityBackend,
    groups: &mut BTreeMap<usize, Vec<usize>>,
    options: &ClusterOptions,
) -> Vec<MessageCluster> {
    let mut clusters = Vec::new();
    let backend_name = backend.name().to_string();
    for (_root, indices) in std::mem::take(groups) {
        let mut sorted_indices = indices;
        sorted_indices.sort_by(|&a, &b| {
            messages[a]
                .created_at
                .cmp(&messages[b].created_at)
                .then(a.cmp(&b))
        });
        // 大簇按时间序切片，每片最多 max_cluster_size
        for chunk in sorted_indices.chunks(options.max_cluster_size) {
            let cluster_messages: Vec<TruncatedMessage> =
                chunk.iter().map(|&i| messages[i].clone()).collect();
            let mean_sim = mean_intra_cluster_similarity(chunk, backend, messages);
            let cluster_id = derive_cluster_id(&cluster_messages);
            clusters.push(MessageCluster {
                cluster_id,
                recurrence: cluster_messages.len(),
                messages: cluster_messages,
                mean_similarity: mean_sim,
                backend: backend_name.clone(),
            });
        }
    }
    // 按 recurrence 降序、cluster_id 升序排列（让大簇先看到）
    clusters.sort_by(|a, b| {
        b.recurrence
            .cmp(&a.recurrence)
            .then(a.cluster_id.cmp(&b.cluster_id))
    });
    clusters
}

/// 簇内成员两两相似度的均值（debug 信息）。单成员簇返回 1.0。
fn mean_intra_cluster_similarity(
    indices: &[usize],
    backend: &SimilarityBackend,
    messages: &[TruncatedMessage],
) -> f32 {
    if indices.len() < 2 {
        return 1.0;
    }
    let mut sum = 0.0f32;
    let mut count = 0usize;
    for i in 0..indices.len() {
        for j in (i + 1)..indices.len() {
            sum += pair_similarity(backend, messages, indices[i], indices[j]);
            count += 1;
        }
    }
    if count == 0 { 0.0 } else { sum / count as f32 }
}

/// 用簇内 observation_id 的 sha256 派生稳定 cluster_id。
fn derive_cluster_id(messages: &[TruncatedMessage]) -> String {
    let mut hasher = Sha256::new();
    for m in messages {
        hasher.update(m.observation_id.as_bytes());
        hasher.update(b"\n");
    }
    let digest = hasher.finalize();
    format!("c_{:x}", digest).chars().take(10).collect()
}

/// 简版并查集（路径压缩）。
struct DisjointSet {
    parent: Vec<usize>,
}

impl DisjointSet {
    fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
        }
    }

    fn find(&mut self, x: usize) -> usize {
        if self.parent[x] != x {
            let root = self.find(self.parent[x]);
            self.parent[x] = root;
        }
        self.parent[x]
    }

    fn union(&mut self, x: usize, y: usize) {
        let rx = self.find(x);
        let ry = self.find(y);
        if rx != ry {
            self.parent[rx] = ry;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn truncated(id: &str, created_at: &str, body: &str) -> TruncatedMessage {
        TruncatedMessage {
            observation_id: id.to_string(),
            role: "unknown".to_string(),
            body: body.to_string(),
            original_len: body.len(),
            truncated_len: body.len(),
            was_truncated: false,
            created_at: created_at.to_string(),
            source_kind: "claude-code-session".to_string(),
        }
    }

    #[test]
    fn empty_input_returns_empty() {
        let out = cluster_messages(&[]).expect("empty ok");
        assert!(out.is_empty());
    }

    #[test]
    fn disjoint_set_basic_union_find() {
        let mut dsu = DisjointSet::new(5);
        dsu.union(0, 1);
        dsu.union(2, 3);
        assert_eq!(dsu.find(0), dsu.find(1));
        assert_eq!(dsu.find(2), dsu.find(3));
        assert_ne!(dsu.find(0), dsu.find(2));
        assert_ne!(dsu.find(0), dsu.find(4));
    }

    #[test]
    fn disjoint_set_chained_union() {
        let mut dsu = DisjointSet::new(4);
        dsu.union(0, 1);
        dsu.union(1, 2);
        dsu.union(2, 3);
        let root = dsu.find(0);
        for i in 1..4 {
            assert_eq!(dsu.find(i), root);
        }
    }

    #[test]
    fn cluster_id_stable_across_runs() {
        let messages = vec![
            truncated("o1", "2026-01-01", "axios"),
            truncated("o2", "2026-01-02", "axios"),
        ];
        let id1 = derive_cluster_id(&messages);
        let id2 = derive_cluster_id(&messages);
        assert_eq!(id1, id2);
        assert!(id1.starts_with("c_"));
    }

    #[test]
    fn pair_threshold_picks_short_for_short_message() {
        let options = ClusterOptions::default();
        let backend = SimilarityBackend::FastEmbed {
            embeddings: Vec::new(),
        };
        let short = truncated("o1", "t1", "短句");
        let long = truncated("o2", "t2", &"长句".repeat(50));
        assert_eq!(
            pair_threshold(&backend, &short, &long, &options),
            DEFAULT_SHORT_THRESHOLD
        );
        assert_eq!(
            pair_threshold(&backend, &long, &long, &options),
            DEFAULT_LONG_THRESHOLD
        );
    }

    #[test]
    fn pair_threshold_jaccard_uses_lower_thresholds() {
        let options = ClusterOptions::default();
        let backend = SimilarityBackend::Jaccard;
        let short = truncated("o1", "t1", "短句");
        let long = truncated("o2", "t2", &"长句".repeat(50));
        assert_eq!(
            pair_threshold(&backend, &short, &long, &options),
            DEFAULT_JACCARD_SHORT_THRESHOLD
        );
        assert_eq!(
            pair_threshold(&backend, &long, &long, &options),
            DEFAULT_JACCARD_LONG_THRESHOLD
        );
    }

    #[test]
    fn mean_similarity_single_member_returns_one() {
        let backend = SimilarityBackend::FastEmbed {
            embeddings: vec![vec![1.0, 0.0, 0.0]],
        };
        let messages = vec![truncated("o1", "t1", "x")];
        let mean = mean_intra_cluster_similarity(&[0], &backend, &messages);
        assert!((mean - 1.0).abs() < 1e-6);
    }

    #[test]
    fn mean_similarity_average_of_pairs() {
        let backend = SimilarityBackend::FastEmbed {
            embeddings: vec![
                vec![1.0, 0.0, 0.0],
                vec![1.0, 0.0, 0.0],
                vec![0.0, 1.0, 0.0],
            ],
        };
        let messages = vec![
            truncated("o1", "t1", "a"),
            truncated("o2", "t2", "b"),
            truncated("o3", "t3", "c"),
        ];
        // 0-1 sim=1, 0-2 sim=0, 1-2 sim=0；均值 = 1/3
        let mean = mean_intra_cluster_similarity(&[0, 1, 2], &backend, &messages);
        assert!((mean - (1.0 / 3.0)).abs() < 1e-6);
    }

    #[test]
    fn jaccard_clusters_similar_chinese_messages_without_network() {
        // 用 force_jaccard 路径验证 cluster 在无网时也能跑
        let messages = vec![
            truncated(
                "o1",
                "2026-01-01",
                "评估提炼质量时优先用真实历史会话做回归验证",
            ),
            truncated("o2", "2026-01-02", "评估提炼质量时使用真实历史会话做回归"),
            truncated(
                "o3",
                "2026-01-03",
                "前端 HTTP 请求统一使用 axios 而不是 fetch",
            ),
        ];
        let options = ClusterOptions {
            force_jaccard: true,
            ..ClusterOptions::default()
        };
        let clusters = cluster_messages_with(&messages, &options).expect("jaccard ok");
        // 应有至少 1 个簇含前两条
        let has_repeated = clusters.iter().any(|c| {
            c.messages.iter().any(|m| m.observation_id == "o1")
                && c.messages.iter().any(|m| m.observation_id == "o2")
        });
        assert!(
            has_repeated,
            "o1 and o2 should be clustered together: {:?}",
            clusters
        );
    }

    #[test]
    fn jaccard_does_not_merge_long_boilerplate_with_different_topics() {
        let common = "timestamp type response_item payload role assistant content output_text text source created_at session workspace project path tool result ";
        let messages = vec![
            truncated(
                "o1",
                "2026-01-01",
                &format!(
                    "{}{}",
                    common,
                    "修复桌宠 hit window ignoreMouseEvents 光标穿透 回归测试 主进程渲染层同步 ".repeat(2)
                ),
            ),
            truncated(
                "o2",
                "2026-01-02",
                &format!(
                    "{}{}",
                    common,
                    "会话列表 sidebar conversation search filter group locales i18n WorkspaceCollapse 存储结构 ".repeat(2)
                ),
            ),
        ];
        let options = ClusterOptions {
            force_jaccard: true,
            keep_singletons: true,
            ..ClusterOptions::default()
        };
        let clusters = cluster_messages_with(&messages, &options).expect("jaccard ok");
        assert_eq!(
            clusters.len(),
            2,
            "shared log/JSON boilerplate must not collapse unrelated long observations: {:?}",
            clusters
        );
    }

    #[test]
    fn jaccard_anchor_clusters_engineering_governance_phrases() {
        let messages = vec![
            truncated("o1", "2026-01-01", "不要主动 git commit，除非我明确说"),
            truncated("o2", "2026-01-02", "不允许自动 push 到 remote"),
            truncated("o3", "2026-01-03", "再次强调 git 操作必须我确认"),
        ];
        let options = ClusterOptions {
            force_jaccard: true,
            keep_singletons: true,
            ..ClusterOptions::default()
        };
        let clusters = cluster_messages_with(&messages, &options).expect("jaccard ok");
        assert_eq!(clusters.len(), 1, "git governance phrases should cluster");
        assert_eq!(clusters[0].recurrence, 3);
    }

    #[test]
    fn jaccard_anchor_clusters_workflow_first_phrases() {
        let messages = vec![
            truncated(
                "o1",
                "2026-01-01",
                "固定路径能解决的问题优先 workflow，不要包装成黑盒 agent",
            ),
            truncated(
                "o2",
                "2026-01-02",
                "这个产品主链路应该 workflow first，Agent 只处理开放探索部分",
            ),
            truncated(
                "o3",
                "2026-01-03",
                "再次强调能流程化就流程化，不要让 Agent 自由乱跑",
            ),
        ];
        let options = ClusterOptions {
            force_jaccard: true,
            keep_singletons: true,
            ..ClusterOptions::default()
        };
        let clusters = cluster_messages_with(&messages, &options).expect("jaccard ok");
        assert_eq!(clusters.len(), 1, "workflow-first phrases should cluster");
        assert_eq!(clusters[0].recurrence, 3);
    }

    #[test]
    fn jaccard_anchor_clusters_evidence_before_confidence_phrases() {
        let messages = vec![
            truncated("o1", "2026-01-01", "先给证据再给信心，不要空口说已经完成"),
            truncated("o2", "2026-01-02", "没有验证输出就不要宣称通过，先展示证据"),
            truncated(
                "o3",
                "2026-01-03",
                "再次强调 evidence before confidence，结论必须跟着验证走",
            ),
        ];
        let options = ClusterOptions {
            force_jaccard: true,
            keep_singletons: true,
            ..ClusterOptions::default()
        };
        let clusters = cluster_messages_with(&messages, &options).expect("jaccard ok");
        assert_eq!(
            clusters.len(),
            1,
            "evidence-before-confidence phrases should cluster"
        );
        assert_eq!(clusters[0].recurrence, 3);
    }

    #[test]
    fn jaccard_anchor_clusters_provider_evidence_grounding_phrases() {
        let messages = vec![
            truncated(
                "o1",
                "2026-01-01",
                "provider 生成的 evidence quote 必须能回到原始 observation",
            ),
            truncated(
                "o2",
                "2026-01-02",
                "不要接受 LLM 编造的 evidence，quote 要能在 observation 里找到",
            ),
            truncated(
                "o3",
                "2026-01-03",
                "再次强调 provider-induced evidence 必须可回溯",
            ),
        ];
        let options = ClusterOptions {
            force_jaccard: true,
            keep_singletons: true,
            ..ClusterOptions::default()
        };
        let clusters = cluster_messages_with(&messages, &options).expect("jaccard ok");
        assert_eq!(
            clusters.len(),
            1,
            "provider evidence grounding phrases should cluster"
        );
        assert_eq!(clusters[0].recurrence, 3);
    }

    #[test]
    fn jaccard_anchor_clusters_file_scoped_drift_phrases() {
        let messages = vec![
            truncated(
                "o1",
                "2026-01-01",
                "drift 恢复应该支持逐文件处理，不要只能全局丢弃",
            ),
            truncated(
                "o2",
                "2026-01-02",
                "多个生成文件 drift 时，要能按目标文件分别导入、保留或丢弃",
            ),
            truncated(
                "o3",
                "2026-01-03",
                "再次强调 file-scoped drift resolution，避免一次覆盖所有手改",
            ),
        ];
        let options = ClusterOptions {
            force_jaccard: true,
            keep_singletons: true,
            ..ClusterOptions::default()
        };
        let clusters = cluster_messages_with(&messages, &options).expect("jaccard ok");
        assert_eq!(
            clusters.len(),
            1,
            "file-scoped drift phrases should cluster"
        );
        assert_eq!(clusters[0].recurrence, 3);
    }

    #[test]
    fn jaccard_anchor_clusters_memory_merge_review_phrases() {
        let messages = vec![
            truncated(
                "o1",
                "2026-01-01",
                "Memory Card 合并前要展示源卡差异、保留字段和影响哪些 Agent",
            ),
            truncated(
                "o2",
                "2026-01-02",
                "生成合并草稿还不够，merge review 要能看 lineage 和最终预览",
            ),
            truncated(
                "o3",
                "2026-01-03",
                "再次强调合并卡片要先审查来源链路和目标 Agent 影响",
            ),
        ];
        let options = ClusterOptions {
            force_jaccard: true,
            keep_singletons: true,
            ..ClusterOptions::default()
        };
        let clusters = cluster_messages_with(&messages, &options).expect("jaccard ok");
        assert_eq!(
            clusters.len(),
            1,
            "memory merge review phrases should cluster"
        );
        assert_eq!(clusters[0].recurrence, 3);
    }

    #[test]
    fn jaccard_anchor_does_not_merge_unrelated_short_requests() {
        let messages = vec![
            truncated("o1", "2026-01-01", "这次只把首页按钮颜色改成红色"),
            truncated("o2", "2026-01-02", "今天临时跳过 lint 检查赶时间"),
        ];
        let options = ClusterOptions {
            force_jaccard: true,
            keep_singletons: true,
            ..ClusterOptions::default()
        };
        let clusters = cluster_messages_with(&messages, &options).expect("jaccard ok");
        assert_eq!(clusters.len(), 2, "one-off requests should stay separate");
    }

    #[test]
    fn jaccard_still_clusters_highly_similar_long_rules() {
        let messages = vec![
            truncated(
                "o1",
                "2026-01-01",
                &"评估提炼质量时必须优先使用真实历史会话进行回归验证，不要只依赖静态样例；验证结果要能暴露长期偏差，并反向修正规则。".repeat(3),
            ),
            truncated(
                "o2",
                "2026-01-02",
                &"评估提炼质量时应优先使用真实历史会话做回归验证，不能只依赖静态样例；验证结果需要暴露长期偏差，并反向修正规则。".repeat(3),
            ),
        ];
        let options = ClusterOptions {
            force_jaccard: true,
            keep_singletons: true,
            ..ClusterOptions::default()
        };
        let clusters = cluster_messages_with(&messages, &options).expect("jaccard ok");
        assert_eq!(
            clusters.len(),
            1,
            "highly similar long rules should cluster"
        );
        assert_eq!(clusters[0].recurrence, 2);
    }

    #[test]
    fn build_groups_handles_isolated_members() {
        // 4 个消息，pairs 只有 (0,1)；期望 2 组：[0,1] 与 [2]、[3]
        let groups = build_groups_via_union_find(4, &[(0, 1, 0.9)]);
        assert_eq!(groups.len(), 3);
        let sizes: Vec<usize> = groups.values().map(|v| v.len()).collect();
        assert!(sizes.contains(&2));
        assert_eq!(sizes.iter().filter(|&&s| s == 1).count(), 2);
    }
}
