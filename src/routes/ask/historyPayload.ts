/**
 * 把助手多轮对话历史构造成发给后端的 payload。
 *
 * assistant 消息里的工具步骤不会被后端直接送入模型，因此这里把已完成步骤
 * 压缩成摘要附在回答末尾，避免下一轮重复走已失败或已经完成的工具路径。
 */

interface HistoryToolStepLike {
  tool?: unknown;
  status?: unknown;
  ok?: unknown;
  hits?: unknown;
  digest?: unknown;
}

interface CompletedHistoryToolStep extends HistoryToolStepLike {
  tool: string;
  status: 'done';
}

interface HistoryMessageLike {
  role?: unknown;
  content?: unknown;
  streaming?: unknown;
  failed?: unknown;
  steps?: unknown;
}

export interface HistoryPayloadEntry {
  role: 'user' | 'assistant';
  content: string;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function isCompletedStep(value: unknown): value is CompletedHistoryToolStep {
  return (
    isRecord(value)
    && value.status === 'done'
    && typeof value.tool === 'string'
    && value.tool.length > 0
  );
}

/** 工具名到摘要片段；仅处理已完成且工具名有效的步骤。 */
function summarizeDoneStep(step: CompletedHistoryToolStep): string {
  const tool = step.tool;
  if (step.ok === false) {
    return `${tool}↯`;
  }
  if (step.ok !== true) {
    return `${tool}?`;
  }
  // 只有 search_memory 的 hits 表示真实引用数量，其他工具只标记成功。
  if (
    tool === 'search_memory'
    && typeof step.hits === 'number'
    && Number.isFinite(step.hits)
    && step.hits >= 0
  ) {
    return `${tool}→${step.hits}条`;
  }
  return `${tool}✓`;
}

export function summarizeStepsForHistory(steps: unknown): string | null {
  if (!Array.isArray(steps) || steps.length === 0) return null;

  const doneSteps = steps.filter(isCompletedStep);
  if (doneSteps.length === 0) return null;

  return `[工具：${doneSteps.map(summarizeDoneStep).join(' | ')}]`;
}

/** 每轮工具结果摘要的字符预算，防止历史膨胀挤掉真实对话。 */
const HISTORY_DIGEST_BUDGET_PER_ROUND = 600;

export function buildStepDigestForHistory(steps: unknown): string | null {
  if (!Array.isArray(steps) || steps.length === 0) return null;

  const lines: string[] = [];
  let used = 0;
  for (const value of steps) {
    if (!isRecord(value)) continue;
    const step: HistoryToolStepLike = value;
    if (step.status !== 'done' || step.ok !== true) continue;
    if (typeof step.digest !== 'string' || !step.digest.trim()) continue;

    const line = `${step.tool as string}: ${step.digest.trim()}`;
    if (used + line.length > HISTORY_DIGEST_BUDGET_PER_ROUND) {
      const remain = HISTORY_DIGEST_BUDGET_PER_ROUND - used;
      if (remain > 40) {
        lines.push(`${line.slice(0, remain)}…`);
      }
      break;
    }
    lines.push(line);
    used += line.length;
  }

  if (lines.length === 0) return null;
  return `[上轮工具数据摘要]\n${lines.join('\n')}`;
}

/**
 * 只保留完整的 user 到 assistant 轮次，并截取最近四轮。
 */
export function buildHistoryPayload(messages: unknown): HistoryPayloadEntry[] {
  const list = Array.isArray(messages) ? messages : [];
  const rounds: [HistoryPayloadEntry, HistoryPayloadEntry][] = [];
  let pendingUser: HistoryMessageLike | null = null;

  for (const value of list) {
    if (!isRecord(value)) continue;
    const message: HistoryMessageLike = value;

    if (message.role === 'user') {
      pendingUser = message;
      continue;
    }

    if (message.role !== 'assistant' || !pendingUser) continue;

    // streaming 使用 truthy 判断，failed 只接受严格 true，保持历史兼容语义。
    if (message.streaming || message.failed === true) {
      pendingUser = null;
      continue;
    }

    const baseContent = String(message.content ?? '');
    const summary = summarizeStepsForHistory(message.steps);
    const digest = buildStepDigestForHistory(message.steps);
    const suffixes = [summary, digest].filter(Boolean).join('\n');
    rounds.push([
      { role: 'user', content: String(pendingUser.content ?? '') },
      {
        role: 'assistant',
        content: suffixes
          ? baseContent
            ? `${baseContent}\n\n${suffixes}`
            : suffixes
          : baseContent,
      },
    ]);
    pendingUser = null;
  }

  return rounds.slice(-4).flat();
}
