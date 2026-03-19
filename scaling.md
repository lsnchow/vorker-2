# Key Design Rules for Agent Swarms (Practical System Guidelines)

## 1. Do NOT scale agents blindly
More agents does not guarantee better performance.
Performance depends on **task structure + coordination architecture**, not agent count.

Rule:
more_agents ≠ better  
correct_topology_for_task = better

---

## 2. Only use swarms for decomposable tasks
Agent swarms work when tasks can be **split into independent subtasks**.

Good swarm tasks:
- parallel research / information gathering
- hypothesis generation
- large search spaces
- independent analysis streams

Pattern:
problem
 ├─ subtask A
 ├─ subtask B
 ├─ subtask C
 └─ synthesis

---

## 3. Avoid swarms for sequential reasoning
If the task requires strict step-by-step reasoning, use a **single agent loop**.

Bad swarm tasks:
step1 → step2 → step3 → step4

Reason:
- context fragmentation
- coordination overhead
- broken reasoning chains

Rule:
sequential_reasoning → single_agent  
parallel_reasoning → swarm

---

## 4. Coordination overhead is the main scaling bottleneck
Communication between agents consumes reasoning capacity.

Costs include:
- message passing
- context compression
- synchronization rounds

Rule:
minimize_inter_agent_messages

---

## 5. Use hierarchical swarm architectures
Large swarms must have centralized coordination.

Best structure:

orchestrator
   │
   ├─ worker agents
   ├─ worker agents
   └─ worker agents

Benefits:
- error containment
- task routing
- result validation

Rule:
large_swarm → hierarchical_control

---

## 6. Avoid independent agent ensembles
Independent agents without coordination perform poorly in agentic systems.

Problems:
- duplicated reasoning
- no error correction
- no shared context

Rule:
avoid_pure_ensembles

---

## 7. Strong base models reduce swarm benefit
As model capability increases, the need for many agents decreases.

Reason:
coordination_cost > marginal_reasoning_gain

Rule:
better_model → fewer_agents_needed

---

## 8. Parallelize exploration, not reasoning chains
Swarms should explore **different parts of a problem**, not debate the same reasoning path.

Good:
- search different sources
- explore repo sections
- generate different hypotheses

Bad:
multiple agents solving the exact same reasoning chain.

---

## 9. Always include centralized validation
A validation or synthesis step is critical.

Pattern:

workers → verifier → final_output

Purpose:
- catch cascading errors
- merge results
- enforce quality checks

---

## 10. Optimize for coordination efficiency
Track system efficiency rather than just success rate.

Important metrics:
- success_per_token
- communication_overhead
- redundancy_between_agents
- error_amplification

Goal:
maximize useful reasoning per unit compute.

---

## Summary Heuristic

Use this decision rule:

IF task is parallelizable:
    use swarm

ELSE IF task is sequential:
    use single agent

AND always:
    keep communication minimal
    enforce hierarchy
    centralize validation
