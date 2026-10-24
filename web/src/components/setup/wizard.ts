import type {
  ReviewValues,
  SetupAnalysis,
  SetupApplyResult,
  SetupPlan,
  SetupScan,
  SetupStatus,
} from '../../api/intake';

/** The steps of the Setup wizard, in order. The evidence step only exists when the scan asks for it. */
export type StepId = 'scan' | 'evidence' | 'analyze' | 'confirm' | 'plan' | 'apply';

export const STEP_ORDER: StepId[] = ['scan', 'evidence', 'analyze', 'confirm', 'plan', 'apply'];

/** Everything the wizard knows about the server session. The server stays the source of truth. */
export interface WizardData {
  scan: SetupScan | null;
  evidenceApproved: boolean;
  /** How many excerpts were approved in this page's lifetime; null after a reload. */
  approvedCount: number | null;
  analysis: SetupAnalysis | null;
  confirmed: ReviewValues | null;
  plan: SetupPlan | null;
  result: SetupApplyResult | null;
  /** The status route said the plan was applied before this page loaded. */
  appliedEarlier: boolean;
}

export const EMPTY_DATA: WizardData = {
  scan: null,
  evidenceApproved: false,
  approvedCount: null,
  analysis: null,
  confirmed: null,
  plan: null,
  result: null,
  appliedEarlier: false,
};

export function needsEvidence(data: WizardData): boolean {
  return data.scan?.evidence.required ?? false;
}

export function stepsFor(data: WizardData): StepId[] {
  return needsEvidence(data) ? STEP_ORDER : STEP_ORDER.filter((id) => id !== 'evidence');
}

/** Conflicts that still need a decision. Approval is blocked while any remain. */
export function unresolvedConflicts(plan: SetupPlan | null): number {
  return plan ? plan.conflicts.filter((c) => c.selected === null).length : 0;
}

export function isApplied(data: WizardData): boolean {
  return data.appliedEarlier || data.result?.status === 'applied';
}

/** The step the person works on now, or `done` once the plan is applied. */
export function currentStep(data: WizardData, editingValues: boolean): StepId | 'done' {
  if (!data.scan) return 'scan';
  if (needsEvidence(data) && !data.evidenceApproved) return 'evidence';
  if (!data.analysis) return 'analyze';
  if (!data.confirmed || editingValues) return 'confirm';
  if (!data.plan || unresolvedConflicts(data.plan) > 0) return 'plan';
  return isApplied(data) ? 'done' : 'apply';
}

/** Twelve characters are enough to compare by eye. The full value goes in a title. */
export function shortHash(hash: string): string {
  return hash.slice(0, 12);
}

const STATUS_RANK: Record<SetupStatus['step'], number> = {
  none: 0,
  scanned: 1,
  evidence_approved: 2,
  analyzed: 3,
  confirmed: 4,
  planned: 5,
  approved: 6,
  applied: 7,
  expired: 0,
};

/** Rebuilds the wizard from GET status, so a reload lands on the right step. */
export function dataFromStatus(status: SetupStatus): { data: WizardData; ended: boolean } {
  if (status.step === 'expired') return { data: EMPTY_DATA, ended: true };
  const rank = STATUS_RANK[status.step];
  return {
    ended: false,
    data: {
      scan: status.scan,
      evidenceApproved: rank >= 2,
      approvedCount: null,
      analysis: rank >= 3 ? status.analysis : null,
      confirmed: rank >= 4 ? status.confirmed_values : null,
      plan: rank >= 5 ? status.plan : null,
      result: null,
      appliedEarlier: status.step === 'applied',
    },
  };
}
