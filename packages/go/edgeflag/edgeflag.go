// Package edgeflag provides a zero-overhead, in-process feature flag evaluation engine and guard SDK.
package edgeflag

import (
	"fmt"
	"hash/fnv"
	"sync"
)

// EvaluationContext encapsulates client context attributes.
type EvaluationContext struct {
	UserID     string
	Attributes map[string]interface{}
}

// TargetingRule defines percentage rollout and conditions.
type TargetingRule struct {
	RolloutPercentage uint8
	Variant           string
}

// FlagDefinition defines a flag with enabled state and rules.
type FlagDefinition struct {
	Key            string
	Enabled        bool
	Rules          []TargetingRule
	DefaultVariant string
}

// EvaluationResult represents flag evaluation outcome.
type EvaluationResult struct {
	FlagKey           string
	Enabled           bool
	Variant           string
	MatchedRuleIndex  int
	Reason            string
}

// ComputeBucket computes deterministic rollout bucket [0..99].
func ComputeBucket(flagKey, userID string) uint8 {
	h := fnv.New64a()
	h.Write([]byte(fmt.Sprintf("%s:%s", flagKey, userID)))
	return uint8(h.Sum64() % 100)
}

// IsInRollout determines if user is within the rollout percentage.
func IsInRollout(flagKey, userID string, percentage uint8) bool {
	if percentage == 0 {
		return false
	}
	if percentage >= 100 {
		return true
	}
	return ComputeBucket(flagKey, userID) < percentage
}

// FlagGuard is the zero-overhead guard interface for Go services.
type FlagGuard interface {
	IsEnabled(flagKey string, ctx EvaluationContext) bool
	Evaluate(flagKey string, ctx EvaluationContext) EvaluationResult
}

// MemoryFlagGuard provides thread-safe in-process memory guard.
type MemoryFlagGuard struct {
	mu    sync.RWMutex
	flags map[string]FlagDefinition
}

// NewMemoryFlagGuard creates a new in-process memory guard.
func NewMemoryFlagGuard() *MemoryFlagGuard {
	return &MemoryFlagGuard{
		flags: make(map[string]FlagDefinition),
	}
}

// SetFlag stores or updates a flag definition.
func (g *MemoryFlagGuard) SetFlag(flag FlagDefinition) {
	g.mu.Lock()
	defer g.mu.Unlock()
	g.flags[flag.Key] = flag
}

// IsEnabled checks if a flag is enabled with zero network hops.
func (g *MemoryFlagGuard) IsEnabled(flagKey string, ctx EvaluationContext) bool {
	return g.Evaluate(flagKey, ctx).Enabled
}

// Evaluate performs in-process rule evaluation.
func (g *MemoryFlagGuard) Evaluate(flagKey string, ctx EvaluationContext) EvaluationResult {
	g.mu.RLock()
	flag, exists := g.flags[flagKey]
	g.mu.RUnlock()

	if !exists || !flag.Enabled {
		reason := "flag_not_found"
		if exists {
			reason = "flag_disabled"
		}
		return EvaluationResult{
			FlagKey: flagKey,
			Enabled: false,
			Reason:  reason,
		}
	}

	for idx, rule := range flag.Rules {
		if IsInRollout(flag.Key, ctx.UserID, rule.RolloutPercentage) {
			variant := rule.Variant
			if variant == "" {
				variant = flag.DefaultVariant
			}
			return EvaluationResult{
				FlagKey:          flag.Key,
				Enabled:          true,
				Variant:          variant,
				MatchedRuleIndex: idx,
				Reason:           fmt.Sprintf("rule_matched_%d", idx),
			}
		}
	}

	return EvaluationResult{
		FlagKey: flag.Key,
		Enabled: true,
		Variant: flag.DefaultVariant,
		Reason:  "default_fallback",
	}
}
