---
name: New Release Template
about: New Release checklist and template
title: 'chore: tag {version}'
labels: ''
assignees: ''

---

# New Release Checklist

 - [ ] Switch to the `main` branch.
 - [ ] `git pull` to ensure the local copy is completely up-to-date.
 - [ ] `git diff origin/main` to ensure there are no local staged or uncommitted changes.
 - [ ] Run local testing (see **Testing** in README) to ensure no artifacts or other local changes that might break tests have been introduced.
 - [ ] Change to the release branch.
    - [ ] If this is a new major/minor release, `git checkout -b release/{major}.{minor}.0` to create a new release branch.
    - [ ] If this is a new patch release:
        - [ ] `git checkout release/{major}.{minor}.{patch}`
        - [ ] `git pull` to ensure the branch is up-to-date.
        - [ ] `git merge main` to merge the new changes into the release branch.
    - Note: For the remainder of this list `{version}` will refer to the `{major}.{minor}.{patch}` you've specified.
- [ ] Edit wherever the version is in source (`Cargo.toml`) so that the version number reflects the desired release version.
- [ ] `clog --setversion {version}`, verify changes were properly accounted for in `CHANGELOG.md`.
- [ ] `git add CHANGELOG.md Cargo.*` to add the changes to the new release commit.
- [ ] `git commit -m "chore: tag {version}"` to commit the new version and record of changes.
- [ ] `git tag -s -m "chore: tag {version}" {version}` to create a signed tag of the current HEAD commit for release.
- [ ] `git push --set-upstream origin release/{version}` to push the commits to a new origin release branch.
- [ ] `git push --tags origin release/{version}` to push the tags to the release branch.
- [ ] Submit a pull request on github to merge the release branch to `main`.
- [ ] Go to the [releases](https://github.com/mozilla-services/contile/releases) page, you should see the new tag with no release information under it.
- [ ] Click the **Draft a new release** button.
- [ ] Enter the version for *Tag version*.
- [ ] Copy/paste the changes from `CHANGELOG.md` into the release description omitting the top 2 lines (the name HTML and the version) of the file. Keep these changes handy, you’ll need them again shortly.
- [ ] Once the release branch pull request is approved and merged, click **Publish Release**.
- [ ] File a bug for [stage deployment in Bugzilla](https://bugzilla.mozilla.org/enter_bug.cgi?assigned_to=nobody%40mozilla.org&bug_status=NEW&bug_type=task&component=Operations%3A%20Miscellaneous&product=Cloud%20Services&short_desc=Please%20deploy%20Contile%20%7Bversion%7D%20to%20stage), in the **Cloud Services** product, under the *Operations: Miscellaneous* component. It should be titled `Please deploy Contile {version} to STAGE` and include the changes in the Description along with any additional instructions to operations regarding deployment changes and special test cases if needed for QA to verify.
