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
