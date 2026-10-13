# GitHub repository setup

This repository is public. The files in `.github/` provide issue forms, a pull request template, dependency update configuration, and a quality workflow. GitHub branch rules are repository settings and must be enabled separately.

## Default-branch ruleset

After the **quality** workflow has completed at least once:

1. Open **Settings → Rules → Rulesets** and create an active branch ruleset for the repository's default branch.
2. Require a pull request before merging.
3. Require the **quality** status check to pass.
4. Require conversation resolution.
5. Block force pushes and branch deletion.
6. If another maintainer can review independently, require one approval. For a single-maintainer workflow, leave approvals optional so the owner can merge their own changes.

GitHub requires the selected status check to have reported recently before it can be selected as a required check. See [creating repository rulesets](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/creating-rulesets-for-a-repository).

## Security and licensing

Enable **Settings → Code security and analysis → Private vulnerability reporting** so the security link in the issue chooser has a private destination. Keep security reports out of public issues.

There is currently no `LICENSE` file. Choose and add a license before inviting reuse or publishing releases; the public visibility of a repository does not itself grant reuse rights.

## Repository presentation

- Set the repository description to: “A desktop workspace for exploring code as a system and keeping project context visible while you work.”
- Add topics that describe the implementation and audience, such as `rust`, `dioxus`, `desktop-app`, and `developer-tools`.
- Keep badges tied to real checks and releases. The README currently shows the quality workflow, live star count, and the Rust/Dioxus stack. Add release, coverage, or license badges only when the corresponding data exists.

GitHub references: [rulesets](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/available-rules-for-rulesets), [issue forms](https://docs.github.com/en/communities/using-templates-to-encourage-useful-issues-and-pull-requests/syntax-for-issue-forms), and [Dependabot configuration](https://docs.github.com/en/code-security/dependabot/dependabot-version-updates/configuration-options-for-the-dependabot.yml-file).
