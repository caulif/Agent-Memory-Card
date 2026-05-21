import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

// 解决 ES module 中 dirname 的问题
const __dirname = path.dirname(fileURLToPath(import.meta.url));

const pages = {
  'app/src/components/pages/Drafts.tsx': [
    'EvalRunPanel',
    'ProjectEvalRunView',
    'ProjectEvalMetricView',
    'selectedCandidateIds',
    'onBatchCandidateAction',
    '批量',
    'visibleDrafts',
    'ArtifactPreviewStrip',
    '[key: string]: any',
    'buildReviewClosureState',
    'review-step',
    'closureState',
    'sourceDrafts',
    'ProjectQualityView',
    'summarizeCandidateEvidence',
    'EvidencePanel',
    'classification-chips',
    'compileLabel',
    'actionLabel',
    'RecordEditor',
    '编辑细则',
    '证据可追溯',
    '待分流',
    '不编译'
  ],
  'app/src/demo/demo-data.ts': [
    '分配矩阵'
  ],
  'app/src/styles/project-detail.css': [
    '分配矩阵'
  ],
  'app/src/components/pages/MemoryCards.tsx': [
    'MemoryGovernancePanel',
    'MemoryGovernanceBatchBar',
    'buildMemoryGovernanceBatchActions',
    'filterMemoryCardsByGovernance',
    'governanceFiltersLabel',
    'pickGovernancePriority',
    '[key: string]: any',
    '📖',
    '⚙️',
    '⚖️'
  ],
  'app/src/components/pages/Agents.tsx': [
    'copyReloadPrompt',
    'reloadPromptStatus',
    'syncAgentFiles',
    '写入-Agent-文件',
    '一键复制重读提示',
    '清空本项目全部分配',
    'GitBranch',
    '[key: string]: any',
    '🎯'
  ],
  'app/src/components/pages/Skills.tsx': [
    'memoryLibrary',
    'quality',
    'pendingAction',
    'onAction',
    'AlertTriangle',
    'Plus',
    'Sparkles',
    'GitBranch',
    '已完成镜像挂载',
    '[key: string]: any'
  ]
};

let hasFailure = false;
console.log('=== UI Focus Contract Audit Report ===\n');

// 1. 审核四大页面组件
for (const [filePath, bannedTerms] of Object.entries(pages)) {
  const resolvedPath = path.resolve(__dirname, '..', filePath.replace('app/', ''));

  if (!fs.existsSync(resolvedPath)) {
    console.error(`Error: File does not exist at ${resolvedPath}`);
    hasFailure = true;
    continue;
  }

  const content = fs.readFileSync(resolvedPath, 'utf8');
  const matchedTerms = [];

  for (const term of bannedTerms) {
    if (content.includes(term)) {
      matchedTerms.push(term);
    }
  }

  if (matchedTerms.length > 0) {
    console.log(`[FAIL] ${filePath}`);
    console.log(`       Found banned terms: ${matchedTerms.map(t => `'${t}'`).join(', ')}\n`);
    hasFailure = true;
  } else {
    console.log(`[PASS] ${filePath}\n`);
  }
}

// 1b. 浏览器演示数据必须展示核心产品判断，不能退回空 Inbox。
const demoPath = path.resolve(__dirname, '../src/demo/demo-data.ts');
if (!fs.existsSync(demoPath)) {
  console.error(`Error: demo-data.ts does not exist at ${demoPath}`);
  hasFailure = true;
} else {
  const demoContent = fs.readFileSync(demoPath, 'utf8');
  const demoRequiredTerms = [
    'skill_targeted_card',
    'already_covered',
    'No Card Is A Success',
    'value_delta',
    'synthesis_trace',
    'search_global_history',
    'summarize_workflow_failures',
    'item_ids'
  ];
  const missingDemoTerms = demoRequiredTerms.filter(term => !demoContent.includes(term));
  if (missingDemoTerms.length > 0) {
    console.log('[FAIL] demo-data.ts: Browser preview must show synthesis decisions, not an empty inbox.');
    console.log(`       Missing required terms: ${missingDemoTerms.map(t => `'${t}'`).join(', ')}\n`);
    hasFailure = true;
  } else {
    console.log('[PASS] demo-data.ts: Preview data demonstrates synthesis value decisions.\n');
  }
}

// 1c. Review Inbox 的兜底候选预览必须像成熟 Memory Card，不能显示内部加工口吻。
const draftsPath = path.resolve(__dirname, '../src/components/pages/Drafts.tsx');
if (!fs.existsSync(draftsPath)) {
  console.error(`Error: Drafts.tsx does not exist at ${draftsPath}`);
  hasFailure = true;
} else {
  const draftsContent = fs.readFileSync(draftsPath, 'utf8');
  const requiredMemoryCardShape = ['目标：', '适用场景：', '执行方式：', '验收：'];
  const missingShapeTerms = requiredMemoryCardShape.filter(term => !draftsContent.includes(term));
  if (missingShapeTerms.length > 0 || draftsContent.includes('用于把')) {
    console.log('[FAIL] Drafts.tsx: Candidate preview must render a mature Memory Card shape.');
    if (missingShapeTerms.length > 0) {
      console.log(`       Missing required shape terms: ${missingShapeTerms.map(t => `'${t}'`).join(', ')}`);
    }
    if (draftsContent.includes('用于把')) {
      console.log("       Found banned internal wording: '用于把'");
    }
    console.log('');
    hasFailure = true;
  } else {
    console.log('[PASS] Drafts.tsx: Candidate fallback preview uses mature Memory Card wording.\n');
  }

  const traceRequiredTerms = [
    'Synthesis Trace',
    '只读证据链',
    '直接证据',
    '更宽历史',
    '失败模式',
    '规则库对照',
    'Skill 对照',
    'item_ids',
    'search_global_history',
    'summarize_workflow_failures'
  ];
  const missingTraceTerms = traceRequiredTerms.filter(term => !draftsContent.includes(term));
  if (missingTraceTerms.length > 0 || draftsContent.toLowerCase().includes('chain-of-thought')) {
    console.log('[FAIL] Drafts.tsx: Review Inbox must show an explainable synthesis trace without chain-of-thought.');
    if (missingTraceTerms.length > 0) {
      console.log(`       Missing required trace terms: ${missingTraceTerms.map(t => `'${t}'`).join(', ')}`);
    }
    if (draftsContent.toLowerCase().includes('chain-of-thought')) {
      console.log("       Found banned wording: 'chain-of-thought'");
    }
    console.log('');
    hasFailure = true;
  } else {
    console.log('[PASS] Drafts.tsx: Review Inbox exposes compact synthesis trace evidence.\n');
  }
}

// 2. 审核主入口 main.tsx 的专注页解耦、杂繁包裹剔除及传参收紧情况
const mainPath = path.resolve(__dirname, '../src/main.tsx');
if (!fs.existsSync(mainPath)) {
  console.error(`Error: main.tsx does not exist at ${mainPath}`);
  hasFailure = true;
} else {
  const mainContent = fs.readFileSync(mainPath, 'utf8');
  const selectionPath = path.resolve(__dirname, '../src/hooks/useProjectSelection.ts');
  const selectionContent = fs.existsSync(selectionPath) ? fs.readFileSync(selectionPath, 'utf8') : '';

  console.log('=== main.tsx Focus Layout Audit ===');

  // a. 校验是否针对 4 大专注页面限制了 ProjectOverviewStrip 的渲染
  const rendersOverviewConditionally = mainContent.includes('!isFocusedPage') && mainContent.includes('ProjectOverviewStrip');
  if (!rendersOverviewConditionally) {
    console.log('[FAIL] main.tsx: ProjectOverviewStrip should only render on non-focused pages (e.g. guarded by !isFocusedPage).');
    hasFailure = true;
  } else {
    console.log('[PASS] main.tsx: ProjectOverviewStrip gating condition ok.');
  }

  // b. 校验是否精简了右侧工作区 inspector，即在 isFocusedPage 下不输出含有右侧 inspector 的 clutter 布局
  // inspector 应该被限制渲染，或者在 isFocusedPage 下不渲染 inspector 容器
  const guardsInspector = mainContent.includes('!isFocusedPage') && mainContent.includes('className="inspector"');
  if (!guardsInspector) {
    console.log('[FAIL] main.tsx: Inspector container (.inspector) should not render on focused pages (e.g. guarded by !isFocusedPage).');
    hasFailure = true;
  } else {
    console.log('[PASS] main.tsx: Inspector container gating condition ok.');
  }

  // c. 校验传给 Drafts 组件的参数，禁止传递无关/已禁用的 props
  const draftsBlock = mainContent.match(/<Drafts[\s\S]*?\/>/)?.[0];
  if (draftsBlock) {
    const bannedDraftsProps = ['evalRun', 'onBatchCandidateAction'];
    const foundProps = bannedDraftsProps.filter(p => draftsBlock.includes(p + '='));
    if (foundProps.length > 0) {
      console.log(`[FAIL] main.tsx <Drafts />: Found extraneous/banned props passed: ${foundProps.map(p => `'${p}'`).join(', ')}`);
      hasFailure = true;
    } else {
      console.log('[PASS] main.tsx <Drafts />: No extraneous props passed.');
    }
  } else {
    console.log('[WARN] main.tsx: Could not find <Drafts /> component block.');
  }

  // d. 校验传给 Skills 组件的参数，只能够传递真正定义过的属性
  const skillsBlock = mainContent.match(/<Skills[\s\S]*?\/>/)?.[0];
  if (skillsBlock) {
    const bannedSkillsProps = ['snapshot', 'memoryLibrary', 'quality', 'pendingAction', 'onAction'];
    const foundProps = bannedSkillsProps.filter(p => skillsBlock.includes(p + '='));
    if (foundProps.length > 0) {
      console.log(`[FAIL] main.tsx <Skills />: Found extraneous/banned props passed: ${foundProps.map(p => `'${p}'`).join(', ')}`);
      hasFailure = true;
    } else {
      console.log('[PASS] main.tsx <Skills />: No extraneous props passed.');
    }
  } else {
    console.log('[WARN] main.tsx: Could not find <Skills /> component block.');
  }

  // e. 校验传给 Agents 组件的参数，去除 quality 和 projects
  const agentsBlock = mainContent.match(/<Agents[\s\S]*?\/>/)?.[0];
  if (agentsBlock) {
    const bannedAgentsProps = ['quality', 'projects'];
    const foundProps = bannedAgentsProps.filter(p => agentsBlock.includes(p + '='));
    if (foundProps.length > 0) {
      console.log(`[FAIL] main.tsx <Agents />: Found extraneous/banned props passed: ${foundProps.map(p => `'${p}'`).join(', ')}`);
      hasFailure = true;
    } else {
      console.log('[PASS] main.tsx <Agents />: No extraneous props passed.');
    }
  } else {
    console.log('[WARN] main.tsx: Could not find <Agents /> component block.');
  }

  // f. 校验浏览器预览模式不通过下一轮 React state 才切换到 demo read models。
  // 否则普通浏览器试用会短暂调用 Tauri invoke，显示“页面数据加载失败”。
  const previewActivationIsExplicit =
    mainContent.includes('options?.previewMode') &&
    mainContent.includes('hydrateReadModelsFromDemo(projectPath, targetPage)') &&
    selectionContent.includes('onProjectActivated(first, page, { previewMode: true })');
  if (!previewActivationIsExplicit) {
    console.log('[FAIL] preview mode: entering browser demo must synchronously hydrate demo read models without a Tauri invoke race.');
    hasFailure = true;
  } else {
    console.log('[PASS] preview mode: demo read-model hydration avoids Tauri invoke race.');
  }
  console.log('');
}

if (hasFailure) {
  console.log('Result: FAILED - Some files violate the clean workspace and strict type focus contracts.');
  process.exit(1);
} else {
  console.log('Result: PASSED - All clean focus contracts met.');
  process.exit(0);
}
