# Memory Card Writing Guide

Use this guide when rewriting extraction candidates into user-reviewable Memory Cards.

## Core Standard

A Memory Card is future instruction, not a summary of a past conversation. It should make a future agent, Skill, or project workflow behave better.

## No Card Is A Success

Do not create a card only because text was extractable. Prefer `ignore`, `already_covered`, or `merge_card` when a new Memory Card would add clutter.

## Required Shape

Every Memory Card proposal needs:

- `Use When` / `触发`: the situation that activates the card.
- `Instructions` / `动作`: the future behavior the agent should follow.
- `Boundaries` / `边界`: where the rule stops, when to ask for review, or what not to infer.
- `value_claim`: what future failure, ambiguity, or friction this card prevents.
- `value_delta`: existing behavior, missing part, new behavior, and why this is not duplicate.

## Skill-Targeted Cards

A Skill-targeted Memory Card improves a project-level Skill. It should state:

- what the Skill already covers
- what the Skill lacks
- how this Memory Card changes future Skill behavior
- whether the card should be mounted as context or fused into the Skill context preview

Do not attach project trivia to a Skill. The card must improve a repeated workflow.

## Duplicate Handling

If an existing Memory Card or Skill already says the same thing, recommend merge or already-covered instead of creating a new card.

## Evidence

Local project evidence is primary. Web evidence may support framing or current facts, but it must not override user-specific preferences.
