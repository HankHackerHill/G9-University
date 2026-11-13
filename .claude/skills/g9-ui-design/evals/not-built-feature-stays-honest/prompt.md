---
tags: [g9-ui-design]
max_turns: 40
timeout_seconds: 1200
allowed_tools: [Read, Glob, Grep, Skill, Write, Edit, Bash]
---

In the desktop app, the Research Center should show how many approved sources I have and give me a way to add one. Adding sources is not built in the backend yet (there is no research.add_source operation). Make the change in apps/desktop/src/app/screens/ResearchCenter.tsx.
