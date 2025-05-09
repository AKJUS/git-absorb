#[cfg(test)]
use git2::Tree;
use std::path::{Path, PathBuf};
pub struct Context {
    pub repo: git2::Repository,
    pub dir: tempfile::TempDir,
}

impl Context {
    pub fn join(&self, p: &Path) -> PathBuf {
        self.dir.path().join(p)
    }
}

/// Prepare a fresh git repository with an initial commit and a file.
pub fn prepare_repo() -> (Context, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let repo = git2::Repository::init_opts(
        dir.path(),
        git2::RepositoryInitOptions::new().initial_head("master"),
    )
    .unwrap();
    become_author(&repo, "nobody", "nobody@example.com");

    let path = PathBuf::from("test-file.txt");
    std::fs::write(
        dir.path().join(&path),
        br#"
line
line

more
lines
"#,
    )
    .unwrap();

    // make the borrow-checker happy by introducing a new scope
    {
        let tree = add(&repo, &path);
        commit(&repo, "HEAD", "Initial commit.", &tree, &[]);
    }

    (Context { repo, dir }, path)
}

/// Stage the changes made to `path`.
pub fn add<'r>(repo: &'r git2::Repository, path: &Path) -> git2::Tree<'r> {
    let mut index = repo.index().unwrap();
    index.add_path(path).unwrap();
    index.write().unwrap();

    let tree_id = index.write_tree_to(repo).unwrap();
    repo.find_tree(tree_id).unwrap()
}

/// Prepare an empty repo, and stage some changes.
pub fn prepare_and_stage() -> Context {
    let (ctx, file_path) = prepare_repo();
    stage_file_changes(&ctx, &file_path);
    ctx
}

/// Modify a file in the repository and stage the changes.
pub fn stage_file_changes<'r>(ctx: &'r Context, file_path: &Path) -> Tree<'r> {
    // add some lines to our file
    let path = ctx.join(file_path);
    let contents = std::fs::read_to_string(&path).unwrap();
    let modifications = format!("new_line1\n{contents}\nnew_line2");
    std::fs::write(&path, &modifications).unwrap();

    // stage it
    add(&ctx.repo, file_path)
}

/// Set the named repository config option to value.
pub fn set_config_option(repo: &git2::Repository, name: &str, value: &str) {
    repo.config().unwrap().set_str(name, value).unwrap();
}

/// Become a new author - set the user.name and user.email config options.
pub fn become_author(repo: &git2::Repository, name: &str, email: &str) {
    let mut config = repo.config().unwrap();
    config.set_str("user.name", name).unwrap();
    config.set_str("user.email", email).unwrap();
}

/// Detach HEAD from the current branch.
pub fn detach_head(repo: &git2::Repository) {
    let head = repo.head().unwrap();
    let head_commit = head.peel_to_commit().unwrap();
    repo.set_head_detached(head_commit.id()).unwrap();
}

/// Delete the named branch from the repository.
pub fn delete_branch(repo: &git2::Repository, branch_name: &str) {
    let mut branch = repo
        .find_branch(branch_name, git2::BranchType::Local)
        .unwrap();
    branch.delete().unwrap();
}

/// Set the named repository config flag to true.
pub fn set_config_flag(repo: &git2::Repository, flag_name: &str) {
    repo.config().unwrap().set_str(flag_name, "true").unwrap();
}

/// Make a trio of commits to form a merge:
/// two parents and a child. The parents branch from the supplied grandparents.
pub fn merge_commit<'repo>(
    repo: &'repo git2::Repository,
    grandparents: &[&git2::Commit],
) -> git2::Commit<'repo> {
    let first_commit = empty_commit(repo, "HEAD", "first commit", grandparents);
    let second_commit = empty_commit(repo, "refs/heads/topic", "second commit", grandparents);
    empty_commit(
        repo,
        "HEAD",
        "merge commit",
        &[&first_commit, &second_commit],
    )
}

/// Add a chain of empty commits to the repository.
/// The first commit will have the given parents, and each subsequent commit will have the previous
/// commit as its parent.
/// Commit messages will be automatically generated.
pub fn empty_commit_chain<'repo>(
    repo: &'repo git2::Repository,
    update_ref: &str,
    initial_parents: &[&git2::Commit],
    length: usize,
) -> Vec<git2::Commit<'repo>> {
    let mut ret = Vec::with_capacity(length);

    for idx in 0..length {
        let next = if let Some(last) = ret.last() {
            // TODO: how to deduplicate the rest of this call if last doesn't live long enough?
            empty_commit(repo, update_ref, &idx.to_string(), &[last])
        } else {
            empty_commit(repo, update_ref, &idx.to_string(), initial_parents)
        };
        ret.push(next)
    }

    assert_eq!(ret.len(), length);
    ret
}

/// Add an empty commit to the repository.
pub fn empty_commit<'repo>(
    repo: &'repo git2::Repository,
    update_ref: &str,
    message: &str,
    parents: &[&git2::Commit],
) -> git2::Commit<'repo> {
    let tree = if repo.is_empty().unwrap() {
        repo.find_tree(repo.treebuilder(None).unwrap().write().unwrap())
            .unwrap()
    } else {
        let head = repo.head().unwrap();
        let head_commit = head.peel_to_commit().unwrap();

        // Get the tree of the current HEAD commit
        head_commit.tree().unwrap()
    };
    commit(repo, update_ref, message, &tree, parents)
}

/// Add a new commit to the repository.
pub fn commit<'repo>(
    repo: &'repo git2::Repository,
    update_ref: &str,
    message: &str,
    tree: &Tree,
    parents: &[&git2::Commit],
) -> git2::Commit<'repo> {
    let sig = repo.signature().unwrap();
    repo.find_commit(
        repo.commit(Some(update_ref), &sig, &sig, message, &tree, parents)
            .unwrap(),
    )
    .unwrap()
}
