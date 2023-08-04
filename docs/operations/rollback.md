# How to Rollback Changes

Note: We use "roll-forward" strategy for rolling back changes in production.

1. Depending on the severity of the problem, decide if this warrants
   [kicking off an incident][incident_docs];
2. Identify the problematic commit and create a revert PR.
   If it is the latest commit, you can revert the change with:
   ```
   git revert HEAD~1
   ```
3. Create a revert PR and go through normal review process to merge PR.

[incident_docs]: https://mozilla-hub.atlassian.net/wiki/spaces/MIR/overview
