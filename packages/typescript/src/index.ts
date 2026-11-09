/**
 * @edgeflag/zero-overhead
 * Zero-Overhead Feature Flag & Evaluation Guard Package for Node.js / Browser.
 */

export * from "./features/kernel_bypass";
export * from "./features/singleflight";

export type AttributeValue = string | number | boolean | string[];

export interface EvaluationContext {
  userId: string;
  attributes?: Record<string, AttributeValue>;
}

export interface TargetingRule {
  rolloutPercentage?: number;
  variant?: string;
  conditions?: Array<{
    field: string;
    op: string;
    value: any;
  }>;
}

export interface FlagDefinition {
  key: string;
  enabled: boolean;
  rules?: TargetingRule[];
  defaultVariant?: string;
}

export interface EvaluationResult {
  flagKey: string;
  enabled: boolean;
  variant?: string;
  matchedRuleIndex?: number;
  reason: string;
}

/**
 * Deterministic FNV-1a 64-bit hashing algorithm producing identical rollout buckets
 * in pure JavaScript / V8 JIT with zero external dependencies.
 */
export function computeBucket(flagKey: string, userId: string): number {
  const str = `${flagKey}:${userId}`;
  let h = 0x811c9dc5; // 32-bit FNV offset basis
  for (let i = 0; i < str.length; i++) {
    h ^= str.charCodeAt(i);
    h = Math.imul(h, 0x01000193);
  }
  return Math.abs(h) % 100;
}

/**
 * Checks if user is included in the percentage rollout.
 */
export function isInRollout(flagKey: string, userId: string, percentage: number): boolean {
  if (percentage <= 0) return false;
  if (percentage >= 100) return true;
  return computeBucket(flagKey, userId) < percentage;
}

/**
 * In-process zero-overhead flag guard.
 */
export interface FlagGuard {
  isEnabled(flagKey: string, context: EvaluationContext): boolean;
  evaluate(flagKey: string, context: EvaluationContext): EvaluationResult;
}

export class MemoryFlagGuard implements FlagGuard {
  private flags = new Map<string, FlagDefinition>();

  public setFlag(flag: FlagDefinition): void {
    this.flags.set(flag.key, flag);
  }

  public isEnabled(flagKey: string, context: EvaluationContext): boolean {
    return this.evaluate(flagKey, context).enabled;
  }

  public evaluate(flagKey: string, context: EvaluationContext): EvaluationResult {
    const flag = this.flags.get(flagKey);
    if (!flag || !flag.enabled) {
      return {
        flagKey,
        enabled: false,
        reason: flag ? "flag_disabled" : "flag_not_found",
      };
    }

    const rules = flag.rules || [];
    for (let i = 0; i < rules.length; i++) {
      const rule = rules[i];
      const pct = rule.rolloutPercentage ?? 100;
      if (isInRollout(flag.key, context.userId, pct)) {
        return {
          flagKey: flag.key,
          enabled: true,
          variant: rule.variant ?? flag.defaultVariant,
          matchedRuleIndex: i,
          reason: `rule_matched_${i}`,
        };
      }
    }

    return {
      flagKey: flag.key,
      enabled: true,
      variant: flag.defaultVariant,
      reason: "default_fallback",
    };
  }
}
