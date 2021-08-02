# Contributing

## Ways to contribute

Contributions can be made in a number of ways:

- Write documentation for the [individual components][readme] of the suite 📝
- Submit a new [issue][new_issue] to propose a change or report a bug 🐛
- Contribute code by creating a [new pull request][new_pull_request] 🚀
- Review [pull requests][open_pull_requests] and provide constructive feedback 💬

## Code of Conduct

This repository is governed by Mozilla's code of conduct and etiquette
guidelines. For more details, please read the [Code of Conduct][coc].

## Guidelines

We additionaly ask you to follow the guidelines below when contributing to
**contile-integration-tests**. 🤖

### Labels

When creating a new issue or pull request make sure to assign appropriate
[labels][labels].

### Milestones

Please create [milestones][milestones] for groups of tasks and set a due date where it makes
sense. When creating a new issue or pull request make sure to set a milestone when
applicable.

### Branch names

Use lowercase alphanumeric characters with hyphens as a separator.

Example:

```text
validation-for-sub2-parameter
```

### Commit messages

Please use the following format in your commit messages: Describe the changes in
a single sentence. Start with an uppercase verb. If you wish to provide
additional information in your commit, please add that in a separate paragraph.
This way the Git history can be parsed more easily on GitHub and on the CLI.

Example:

```text
Add parameterized unit test for sub2 validation

The unit test uses a hyphenated parameter, a parameter that exceeds the maximum
character count, and a parameter using emoji characters.
```

### Tag names

Versions follow [Calendar Versioning][calver] using a `YY.MINOR.MICRO` scheme. 🗓

Example:

```text
21.2.0
```

[coc]: ../CODE_OF_CONDUCT.md
[readme]: ../README.md
[new_issue]: https://github.com/mozilla-services/contile-integration-tests/issues/new
[new_pull_request]: https://github.com/mozilla-services/contile-integration-tests/compare
[open_pull_requests]: https://github.com/mozilla-services/contile-integration-tests/pulls
[labels]: https://github.com/mozilla-services/contile-integration-tests/labels
[milestones]: https://github.com/mozilla-services/contile-integration-tests/milestones
[calver]: https://calver.org
