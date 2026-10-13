# GitHub repository setup

This guide takes you from the repository files already in place to the settings GitHub needs. First, let the quality workflow run once. After that, make its check required in a ruleset for the default branch. Then review the security and presentation settings below. The [development guide](DEVELOPMENT.md) lists the checks run by the workflow.

This repository is public. The files in `.github/` provide issue forms, a pull request template, dependency update configuration, and a quality workflow. GitHub branch rules are repository settings and must be enabled separately.

## Default-branch ruleset

First, wait for the **quality** workflow to complete successfully at least once. GitHub needs to see the check before it can be selected as required. Then:

1. Open **Settings → Rules → Rulesets** and create an active branch ruleset for the repository's default branch.
2. Require a pull request before merging.
3. Require the **quality** status check to pass.
4. Require conversation resolution.
5. Block force pushes and branch deletion.
6. If another maintainer can review independently, require one approval. For a single-maintainer workflow, leave approvals optional so the owner can merge their own changes.

GitHub requires the selected status check to have reported recently before it can be selected as a required check. See [creating repository rulesets](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/creating-rulesets-for-a-repository).

## Security and licensing

Enable **Settings → Code security and analysis → Private vulnerability reporting**. After you enable it, the security link in the issue chooser has a private destination. Ask people to report security issues there instead of posting them in public issues.

There is currently no `LICENSE` file. Before inviting people to reuse the code or publishing releases, choose and add a license. A public repository can be viewed, but public visibility alone does not give others permission to reuse the code.

## Repository presentation

- Set the repository description to: “A desktop workspace for exploring code as a system and keeping project context visible while you work.”
- Add topics that describe the implementation and audience, such as `rust`, `dioxus`, `desktop-app`, and `developer-tools`.
- Keep each badge connected to information the repository actually provides. The README shows the quality workflow, live star count, and Rust/Dioxus stack. Add a release, coverage, or license badge only after that information exists and its source is set up.

GitHub references: [rulesets](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/available-rules-for-rulesets), [issue forms](https://docs.github.com/en/communities/using-templates-to-encourage-useful-issues-and-pull-requests/syntax-for-issue-forms), and [Dependabot configuration](https://docs.github.com/en/code-security/dependabot/dependabot-version-updates/configuration-options-for-the-dependabot.yml-file).
