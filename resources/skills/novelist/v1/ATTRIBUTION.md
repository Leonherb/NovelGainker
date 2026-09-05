# Third-party attribution

This Skill Pack is adapted from the public [comfuture/novelist](https://github.com/comfuture/novelist) repository at commit `64e4a2e1473ff0bad5893eff394ccbf1fa1cd580`.

Copyright (c) 2026 Changkyun Kim. Upstream content is provided under the MIT License; the exact license text is stored in `LICENSE.upstream`.

NovelGainker adaptations:

- replace host-specific commands and executable script flows with app-owned, provider-neutral capability instructions and bounded Host tools;
- use Simplified Chinese safety contracts and NovelGainker-native compatibility fixtures;
- exclude Python, host adapters, Agent YAML, Korean-only defaults, website and submission metadata;
- preserve selected upstream storytelling references verbatim and record their source hashes in `provenance.lock.json`.

Projection 1.1.0 adds the approved NR-4 five-type maintenance instructions and an application-authored outline template. The upstream version, commit and archive identity are unchanged. Write authority comes only from the current Host tool contract, never from a resource.

The runtime validates this repository snapshot and embeds it into the application. It never loads the upstream repository, a user Skill file, an external plugin cache or the network at runtime.
