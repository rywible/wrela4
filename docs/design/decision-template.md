# {{NUMBER}}: {{TITLE}}

Date: {{YYYY-MM-DD}}

## Status

Proposed | Accepted | Superseded by {{OTHER}}

## Context

{{WHY_THIS_DECISION_MATTERS}}

## Decision

{{WHAT_WE_ARE_DOING}}

## Dependency Decision Rule

If this ADR introduces an external crate, answer:

- What problem is painful enough to justify the dependency?
- Why is handwritten code no longer the right tradeoff?
- What compile-time cost does the dependency add?
- What transitive dependencies does it bring?
- Does it affect determinism, bootstrapping, portability, or audit surface?
- What is the exit plan if the dependency becomes a burden?

## Consequences

### Positive

- {{BENEFIT}}

### Negative

- {{COST}}

## Alternatives Considered

- {{ALTERNATIVE}} — rejected because {{REASON}}

## References

- [`docs/design-principles.md`](../design-principles.md)
- Related plans: `docs/implementation/plans/...`
