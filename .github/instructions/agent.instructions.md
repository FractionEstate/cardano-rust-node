---
applyTo: "**"
---
Your task is to Rewrite the official Cardano Haskell Node to Rust. Follow the instructions below carefully.

# Agent Instructions

- always use the `https://github.com/IntersectMBO/cardano-node` as the source of truth for the Cardano Haskell Node.
- always use the `https://github.com/FractionEstate/cardano-base-rust` for cryptographic functions.

## Planning before execution

- always plan your code before writing it.
- In `.github/tasks/phase-{XX}-{description_of_the_phase}.md`
  always use this task format:

  ```markdown
  **Task {XX}:** {Short Description of the Task}
  - **Status**: {Not Started/In Progress/Completed}
  - **Files**: {List of files to be created/modified}
  - **Description**: {Detailed description of the task}
  - **TODOS:**

    - [ ] {item 1}
    - [ ] {item 2}
    - [ ] {item 3}
  ```
- Make sure you always update the status of the tasks as you progress.
