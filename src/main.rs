use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::env;
use std::fs;
use std::io::{self, IsTerminal};
use std::path::{Component, Path, PathBuf};
use std::process::{self, Command};
use std::time::{SystemTime, UNIX_EPOCH};

type Result<T> = std::result::Result<T, String>;

const BLUE: &str = "141;211;247";
const PINK: &str = "245;194;231";
const YELLOW: &str = "249;226;175";
const PROGRAM_NAME: &str = "casa";
const WORKSPACE_ENV_VAR: &str = "AICASA_ROOT";
const TRASH_ENV_VAR: &str = "AICASA_TRASH_DIR";
const WORKSPACE_DIRECTORY: &str = ".aicasa";
const METADATA_FILE: &str = ".aicasa.json";
const METADATA_SCHEMA_VERSION: u32 = 1;
const BANNER_COLORS: [&str; 7] = [BLUE, PINK, YELLOW, BLUE, PINK, YELLOW, BLUE];
const BANNER_LETTERS: [[&str; 6]; 7] = [
    [
        " █████╗ ",
        "██╔══██╗",
        "███████║",
        "██╔══██║",
        "██║  ██║",
        "╚═╝  ╚═╝",
    ],
    ["██╗", "██║", "██║", "██║", "██║", "╚═╝"],
    [
        " ██████╗",
        "██╔════╝",
        "██║     ",
        "██║     ",
        "╚██████╗",
        " ╚═════╝",
    ],
    [
        " █████╗ ",
        "██╔══██╗",
        "███████║",
        "██╔══██║",
        "██║  ██║",
        "╚═╝  ╚═╝",
    ],
    [
        "███████╗",
        "██╔════╝",
        "███████╗",
        "╚════██║",
        "███████║",
        "╚══════╝",
    ],
    [
        " █████╗ ",
        "██╔══██╗",
        "███████║",
        "██╔══██║",
        "██║  ██║",
        "╚═╝  ╚═╝",
    ],
    [
        " █████╗ ",
        "██╔══██╗",
        "██║  ╚═╝",
        "██║  ██╗",
        "╚█████╔╝",
        " ╚════╝ ",
    ],
];

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if let Err(error) = run(&args) {
        let theme = Theme::stderr();
        eprintln!("{}", theme.pink(&format!("error: {error}")));
        process::exit(1);
    }
}

fn run(args: &[String]) -> Result<()> {
    let Some(command) = args.first().map(String::as_str) else {
        print_help();
        return Ok(());
    };

    match command {
        "new" => command_new(&args[1..]),
        "add" => command_add(&args[1..]),
        "rm" => command_rm(&args[1..]),
        "ls" => command_ls(&args[1..]),
        "inspect" => command_inspect(&args[1..]),
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        "--version" | "-V" => {
            println!("{PROGRAM_NAME} {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        unknown => Err(format!(
            "unknown command `{unknown}`. Run `{PROGRAM_NAME} --help` for usage."
        )),
    }
}

fn command_new(args: &[String]) -> Result<()> {
    let mut args = args.to_vec();
    let print_path = take_flag(&mut args, "--print-path");
    if args.len() < 2 {
        return Err(format!(
            "usage: {PROGRAM_NAME} new <project> <owner/repo[,owner/repo...] | git-url...>"
        ));
    }

    let project = &args[0];
    validate_project_name(project)?;
    let repositories = parse_repositories(&args[1..])?;
    let root = workspace_root()?;
    let printer = Printer::new(if print_path {
        Destination::Stderr
    } else {
        Destination::Stdout
    });
    let path = create_project(&root, project, &repositories, &printer)?;

    if print_path {
        println!("{}", path.display());
    } else {
        printer.status(format!(
            "{} {}",
            printer.theme.yellow("Workspace ready:"),
            printer.theme.blue(&path.display().to_string())
        ));
        printer.status(format!(
            "Enter it with: {}",
            printer
                .theme
                .blue(&format!("cd {}", shell_quote(&path.display().to_string())))
        ));
    }

    Ok(())
}

fn command_add(args: &[String]) -> Result<()> {
    if args.is_empty() {
        return Err(format!(
            "usage: {PROGRAM_NAME} add [<project>] <owner/repo[,owner/repo...] | git-url...>"
        ));
    }

    let root = workspace_root()?;
    let (project_path, repository_args) = find_add_target(&root, args)?;
    let repositories = parse_repositories(repository_args)?;
    let printer = Printer::new(Destination::Stdout);
    add_repositories(&project_path, &repositories, &printer)?;
    printer.status(format!(
        "{} {}",
        printer.theme.yellow("Updated workspace:"),
        printer.theme.blue(&project_path.display().to_string())
    ));
    Ok(())
}

fn command_rm(args: &[String]) -> Result<()> {
    if args.is_empty() {
        return Err(format!("usage: {PROGRAM_NAME} rm <project> [project...]"));
    }

    for project in args {
        validate_project_name(project)?;
    }

    let root = workspace_root()?;
    let trash = trash_directory()?;
    let printer = Printer::new(Destination::Stdout);
    move_to_trash(&root, &trash, args, &printer)?;
    Ok(())
}

fn command_ls(args: &[String]) -> Result<()> {
    if !args.is_empty() {
        return Err(format!("usage: {PROGRAM_NAME} ls"));
    }

    let root = workspace_root()?;
    let entries = list_projects(&root)?;
    let theme = Theme::stdout();
    if entries.is_empty() {
            println!(
                "{}",
                theme.yellow(&format!(
                    "No workspaces found. Create one with `{PROGRAM_NAME} new <project> <owner/repo>`."
                ))
            );
        return Ok(());
    }

    println!("{}", theme.pink("Workspaces"));
    for entry in entries {
        let repository_label = if entry.repositories == 1 {
            "1 repo".to_string()
        } else {
            format!("{} repos", entry.repositories)
        };
        let padded_name = format!("{:<24}", entry.name);
        let padded_repository_label = format!("{repository_label:<12}");
        println!(
            "  {} {} {}",
            theme.blue(&padded_name),
            theme.yellow(&padded_repository_label),
            entry.path.display()
        );
    }
    Ok(())
}

fn command_inspect(args: &[String]) -> Result<()> {
    let root = workspace_root()?;
    let project_path = match args {
        [] => current_workspace(&root).ok_or_else(|| {
            format!(
                "outside an {PROGRAM_NAME} workspace, specify the target: `{PROGRAM_NAME} inspect <project>`."
            )
        })?,
        [project] => {
            validate_project_name(project)?;
            let project_path = root.join(project);
            if !project_path.is_dir() {
                return Err(format!("workspace `{project}` does not exist."));
            }
            project_path
        }
        _ => return Err(format!("usage: {PROGRAM_NAME} inspect [<project>]")),
    };

    let inspection = inspect_workspace(&project_path)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&inspection)
            .map_err(|error| format!("could not encode workspace inspection: {error}"))?
    );
    Ok(())
}

fn print_help() {
    let theme = Theme::stdout();
    print_banner(&theme);
    println!("{}", theme.pink("casa - AI-assisted project workspaces"));
    println!();
    println!("{}", theme.yellow("Usage"));
    print_help_line(
        &theme,
        "casa new <project> <repos>",
        "Create a workspace and clone repositories",
    );
    print_help_line(
        &theme,
        "casa add [project] <repos>",
        "Clone more repositories into a workspace",
    );
    print_help_line(
        &theme,
        "casa rm <project> [project...]",
        "Move workspaces to the Trash",
    );
    print_help_line(
        &theme,
        "casa ls",
        &format!("List workspaces in ~/{WORKSPACE_DIRECTORY}"),
    );
    print_help_line(
        &theme,
        "casa inspect [project]",
        "Print machine-readable workspace metadata",
    );
    println!();
    println!("{}", theme.yellow("Examples"));
    println!("  casa new new-project paradise-runner/toast,paradise-runner/kaleidoscope");
    println!("  casa add new-project paradise-runner/another-repo");
    println!("  casa inspect new-project");
}

fn print_help_line(theme: &Theme, invocation: &str, description: &str) {
    println!(
        "  {} {description}",
        theme.blue(&format!("{invocation:<34}"))
    );
}

fn render_banner(theme: &Theme) -> String {
    let mut banner = String::new();
    for row in 0..BANNER_LETTERS[0].len() {
        for (letter, color) in BANNER_LETTERS.iter().zip(BANNER_COLORS) {
            banner.push_str(&theme.banner_letter(letter[row], color));
        }
        banner.push('\n');
    }
    banner
}

fn print_banner(theme: &Theme) {
    println!("{}", render_banner(theme));
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Repository {
    source: String,
    directory: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct StoredRepository {
    source: Option<String>,
    directory: String,
}

impl From<&Repository> for StoredRepository {
    fn from(repository: &Repository) -> Self {
        Self {
            source: Some(repository.source.clone()),
            directory: repository.directory.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct WorkspaceMetadata {
    schema_version: u32,
    name: String,
    repositories: Vec<StoredRepository>,
}

impl WorkspaceMetadata {
    fn empty(name: &str) -> Self {
        Self {
            schema_version: METADATA_SCHEMA_VERSION,
            name: name.to_string(),
            repositories: Vec::new(),
        }
    }
}

#[derive(Debug, Serialize)]
struct WorkspaceInspection {
    schema_version: u32,
    name: String,
    path: PathBuf,
    metadata_path: PathBuf,
    metadata_present: bool,
    repositories: Vec<InspectedRepository>,
}

#[derive(Debug, Serialize)]
struct InspectedRepository {
    directory: String,
    source: Option<String>,
    path: PathBuf,
    exists: bool,
}

fn parse_repositories(args: &[String]) -> Result<Vec<Repository>> {
    let specifications: Vec<&str> = args
        .iter()
        .flat_map(|argument| argument.split(','))
        .map(str::trim)
        .filter(|specification| !specification.is_empty())
        .collect();
    if specifications.is_empty() {
        return Err("at least one repository is required.".to_string());
    }

    specifications
        .into_iter()
        .map(repository_from_specification)
        .collect()
}

fn repository_from_specification(specification: &str) -> Result<Repository> {
    let is_github_shorthand = {
        let segments: Vec<&str> = specification.split('/').collect();
        segments.len() == 2
            && segments.iter().all(|segment| !segment.is_empty())
            && !specification.contains(':')
            && !specification.starts_with('.')
            && !Path::new(specification).is_absolute()
    };

    let source = if is_github_shorthand {
        let without_suffix = specification.strip_suffix(".git").unwrap_or(specification);
        format!("https://github.com/{without_suffix}.git")
    } else {
        specification.to_string()
    };
    let trimmed = specification.trim_end_matches('/');
    let directory = trimmed
        .rsplit(['/', ':'])
        .next()
        .unwrap_or_default()
        .strip_suffix(".git")
        .unwrap_or_else(|| trimmed.rsplit(['/', ':']).next().unwrap_or_default())
        .to_string();
    validate_repository_directory(&directory, specification)?;

    Ok(Repository { source, directory })
}

fn validate_repository_directory(directory: &str, specification: &str) -> Result<()> {
    if directory.is_empty()
        || directory == "."
        || directory == ".."
        || directory.contains('/')
        || directory.contains('\\')
    {
        return Err(format!(
            "cannot determine a checkout directory from repository `{specification}`."
        ));
    }
    Ok(())
}

fn validate_project_name(project: &str) -> Result<()> {
    if project.is_empty() {
        return Err("project name cannot be empty.".to_string());
    }
    let mut components = Path::new(project).components();
    if !matches!(components.next(), Some(Component::Normal(_)))
        || components.next().is_some()
        || project.contains('\\')
    {
        return Err(format!(
            "invalid project name `{project}`; use a single directory name."
        ));
    }
    Ok(())
}

fn create_project(
    root: &Path,
    project: &str,
    repositories: &[Repository],
    printer: &Printer,
) -> Result<PathBuf> {
    let project_path = root.join(project);
    if project_path.exists() {
        return Err(format!("workspace `{project}` already exists."));
    }
    ensure_unique_destinations(&project_path, repositories)?;
    fs::create_dir_all(root)
        .map_err(|error| format!("could not create `{}`: {error}", root.display()))?;
    fs::create_dir(&project_path).map_err(|error| {
        format!(
            "could not create workspace `{}`: {error}",
            project_path.display()
        )
    })?;
    printer.status(format!(
        "{} {}",
        printer.theme.pink("Created workspace"),
        printer.theme.blue(&project_path.display().to_string())
    ));
    let mut metadata = WorkspaceMetadata::empty(project);
    write_workspace_metadata(&project_path, &metadata)?;
    clone_repositories(
        &project_path,
        repositories,
        Some(project),
        &mut metadata,
        printer,
    )?;
    Ok(project_path)
}

fn add_repositories(
    project_path: &Path,
    repositories: &[Repository],
    printer: &Printer,
) -> Result<()> {
    if !project_path.is_dir() {
        return Err(format!(
            "workspace `{}` does not exist.",
            project_path.display()
        ));
    }
    let (mut metadata, _) = load_or_infer_metadata(project_path)?;
    ensure_unique_destinations(project_path, repositories)?;
    clone_repositories(project_path, repositories, None, &mut metadata, printer)
}

fn ensure_unique_destinations(project_path: &Path, repositories: &[Repository]) -> Result<()> {
    let mut destinations = HashSet::new();
    for repository in repositories {
        if !destinations.insert(&repository.directory) {
            return Err(format!(
                "repository directory `{}` was requested more than once.",
                repository.directory
            ));
        }
        if project_path.join(&repository.directory).exists() {
            return Err(format!(
                "repository directory `{}` already exists in `{}`.",
                repository.directory,
                project_path.display()
            ));
        }
    }
    Ok(())
}

fn clone_repositories(
    project_path: &Path,
    repositories: &[Repository],
    branch_name: Option<&str>,
    metadata: &mut WorkspaceMetadata,
    printer: &Printer,
) -> Result<()> {
    for repository in repositories {
        let destination = project_path.join(&repository.directory);
        printer.status(format!(
            "{} {} {} {}",
            printer.theme.pink("Cloning"),
            printer.theme.blue(&repository.source),
            printer.theme.yellow("into"),
            destination.display()
        ));
        let status = Command::new("git")
            .arg("clone")
            .arg("--")
            .arg(&repository.source)
            .arg(&destination)
            .status()
            .map_err(|error| format!("could not execute `git clone`: {error}"))?;
        if !status.success() {
            return Err(format!(
                "clone failed for `{}`; workspace remains at `{}`.",
                repository.source,
                project_path.display()
            ));
        }
        if let Some(branch_name) = branch_name {
            create_and_checkout_branch(&destination, branch_name).map_err(|error| {
                format!(
                    "cloned `{}` but could not create branch `{branch_name}`: {error}",
                    repository.source
                )
            })?;
        }
        let stored_repository = StoredRepository::from(repository);
        if let Some(stored) = metadata
            .repositories
            .iter_mut()
            .find(|stored| stored.directory == repository.directory)
        {
            *stored = stored_repository;
        } else {
            metadata.repositories.push(stored_repository);
        }
        write_workspace_metadata(project_path, metadata).map_err(|error| {
            format!(
                "cloned `{}` but could not update workspace metadata: {error}",
                repository.source
            )
        })?;
    }
    Ok(())
}

fn create_and_checkout_branch(repository_path: &Path, branch_name: &str) -> Result<()> {
    let switch_status = Command::new("git")
        .arg("-C")
        .arg(repository_path)
        .arg("switch")
        .arg("-c")
        .arg(branch_name)
        .status()
        .map_err(|error| format!("could not execute `git switch -c`: {error}"))?;
    if switch_status.success() {
        return Ok(());
    }

    let orphan_status = Command::new("git")
        .arg("-C")
        .arg(repository_path)
        .arg("checkout")
        .arg("--orphan")
        .arg(branch_name)
        .status()
        .map_err(|error| format!("could not execute `git checkout --orphan`: {error}"))?;
    if orphan_status.success() {
        Ok(())
    } else {
        Err("git could not create the requested branch.".to_string())
    }
}

fn find_add_target<'a>(root: &Path, args: &'a [String]) -> Result<(PathBuf, &'a [String])> {
    if args.len() >= 2 && validate_project_name(&args[0]).is_ok() {
        let project_path = root.join(&args[0]);
        if !project_path.is_dir() {
            return Err(format!("workspace `{}` does not exist.", args[0]));
        }
        return Ok((project_path, &args[1..]));
    }

    if let Some(project_path) = current_workspace(root) {
        return Ok((project_path, args));
    }

    Err(format!(
        "outside an {PROGRAM_NAME} workspace, specify the target: `{PROGRAM_NAME} add <project> <owner/repo>`."
    ))
}

fn current_workspace(root: &Path) -> Option<PathBuf> {
    let canonical_root = fs::canonicalize(root).ok()?;
    let current = fs::canonicalize(env::current_dir().ok()?).ok()?;
    let relative = current.strip_prefix(&canonical_root).ok()?;
    let name = match relative.components().next()? {
        Component::Normal(name) => name,
        _ => return None,
    };
    let workspace = canonical_root.join(name);
    workspace.is_dir().then_some(workspace)
}

fn inspect_workspace(project_path: &Path) -> Result<WorkspaceInspection> {
    if !project_path.is_dir() {
        return Err(format!(
            "workspace `{}` does not exist.",
            project_path.display()
        ));
    }
    let (metadata, metadata_present) = load_or_infer_metadata(project_path)?;
    let path = fs::canonicalize(project_path).unwrap_or_else(|_| project_path.to_path_buf());
    let repositories = metadata
        .repositories
        .into_iter()
        .map(|repository| {
            let repository_path = path.join(&repository.directory);
            InspectedRepository {
                directory: repository.directory,
                source: repository.source,
                exists: repository_path.is_dir(),
                path: repository_path,
            }
        })
        .collect();
    Ok(WorkspaceInspection {
        schema_version: METADATA_SCHEMA_VERSION,
        name: metadata.name,
        metadata_path: path.join(METADATA_FILE),
        path,
        metadata_present,
        repositories,
    })
}

fn load_or_infer_metadata(project_path: &Path) -> Result<(WorkspaceMetadata, bool)> {
    if let Some(metadata) = read_workspace_metadata(project_path)? {
        return Ok((metadata, true));
    }
    Ok((infer_workspace_metadata(project_path)?, false))
}

fn read_workspace_metadata(project_path: &Path) -> Result<Option<WorkspaceMetadata>> {
    let path = project_path.join(METADATA_FILE);
    if !path.exists() {
        return Ok(None);
    }
    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("could not read metadata `{}`: {error}", path.display()))?;
    let metadata: WorkspaceMetadata = serde_json::from_str(&contents)
        .map_err(|error| format!("could not parse metadata `{}`: {error}", path.display()))?;
    validate_workspace_metadata(project_path, &metadata)?;
    Ok(Some(metadata))
}

fn infer_workspace_metadata(project_path: &Path) -> Result<WorkspaceMetadata> {
    let name = workspace_name(project_path)?;
    let mut repositories = Vec::new();
    let entries = fs::read_dir(project_path)
        .map_err(|error| format!("could not read `{}`: {error}", project_path.display()))?;
    for entry in entries {
        let entry = entry
            .map_err(|error| format!("could not read `{}`: {error}", project_path.display()))?;
        if entry
            .file_type()
            .map_err(|error| format!("could not inspect `{}`: {error}", entry.path().display()))?
            .is_dir()
        {
            let directory = entry
                .file_name()
                .into_string()
                .map_err(|_| "workspace contains a non-UTF-8 directory name.".to_string())?;
            repositories.push(StoredRepository {
                source: None,
                directory,
            });
        }
    }
    repositories.sort_by(|left, right| left.directory.cmp(&right.directory));
    Ok(WorkspaceMetadata {
        schema_version: METADATA_SCHEMA_VERSION,
        name,
        repositories,
    })
}

fn write_workspace_metadata(project_path: &Path, metadata: &WorkspaceMetadata) -> Result<()> {
    validate_workspace_metadata(project_path, metadata)?;
    let path = project_path.join(METADATA_FILE);
    let temporary_path = project_path.join(format!("{METADATA_FILE}.tmp-{}", process::id()));
    let mut serialized = serde_json::to_vec_pretty(metadata)
        .map_err(|error| format!("could not encode metadata `{}`: {error}", path.display()))?;
    serialized.push(b'\n');
    fs::write(&temporary_path, serialized).map_err(|error| {
        format!(
            "could not write metadata `{}`: {error}",
            temporary_path.display()
        )
    })?;
    fs::rename(&temporary_path, &path)
        .map_err(|error| format!("could not replace metadata `{}`: {error}", path.display()))?;
    Ok(())
}

fn validate_workspace_metadata(project_path: &Path, metadata: &WorkspaceMetadata) -> Result<()> {
    if metadata.schema_version != METADATA_SCHEMA_VERSION {
        return Err(format!(
            "unsupported metadata schema version `{}` in `{}`.",
            metadata.schema_version,
            project_path.join(METADATA_FILE).display()
        ));
    }
    let expected_name = workspace_name(project_path)?;
    if metadata.name != expected_name {
        return Err(format!(
            "metadata workspace name `{}` does not match directory `{expected_name}`.",
            metadata.name
        ));
    }

    let mut directories = HashSet::new();
    for repository in &metadata.repositories {
        validate_repository_directory(&repository.directory, &repository.directory)?;
        if repository.source.as_ref().is_some_and(String::is_empty) {
            return Err(format!(
                "repository `{}` has an empty metadata source.",
                repository.directory
            ));
        }
        if !directories.insert(&repository.directory) {
            return Err(format!(
                "repository directory `{}` appears more than once in metadata.",
                repository.directory
            ));
        }
    }
    Ok(())
}

fn workspace_name(project_path: &Path) -> Result<String> {
    project_path
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .ok_or_else(|| {
            format!(
                "could not determine workspace name from `{}`.",
                project_path.display()
            )
        })
}

fn move_to_trash(root: &Path, trash: &Path, projects: &[String], printer: &Printer) -> Result<()> {
    let mut sources = Vec::new();
    let mut requested = HashSet::new();
    for project in projects {
        if !requested.insert(project) {
            return Err(format!(
                "workspace `{project}` was requested more than once."
            ));
        }
        let source = root.join(project);
        if !source.is_dir() {
            return Err(format!("workspace `{project}` does not exist."));
        }
        sources.push((project, source));
    }
    fs::create_dir_all(trash)
        .map_err(|error| format!("could not open Trash at `{}`: {error}", trash.display()))?;

    for (project, source) in sources {
        let destination = unused_trash_path(trash, project);
        fs::rename(&source, &destination).map_err(|error| {
            format!(
                "could not move `{}` to `{}`: {error}",
                source.display(),
                destination.display()
            )
        })?;
        printer.status(format!(
            "{} {}",
            printer.theme.pink("Moved to Trash:"),
            printer.theme.blue(&destination.display().to_string())
        ));
    }
    Ok(())
}

fn unused_trash_path(trash: &Path, project: &str) -> PathBuf {
    let direct = trash.join(project);
    if !direct.exists() {
        return direct;
    }

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    for suffix in 0_u32.. {
        let name = if suffix == 0 {
            format!("{project}-{timestamp}")
        } else {
            format!("{project}-{timestamp}-{suffix}")
        };
        let candidate = trash.join(name);
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!()
}

#[derive(Debug, PartialEq, Eq)]
struct ProjectEntry {
    name: String,
    path: PathBuf,
    repositories: usize,
}

fn list_projects(root: &Path) -> Result<Vec<ProjectEntry>> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let read_dir = fs::read_dir(root)
        .map_err(|error| format!("could not read `{}`: {error}", root.display()))?;
    let mut entries = Vec::new();
    for entry in read_dir {
        let entry =
            entry.map_err(|error| format!("could not read `{}`: {error}", root.display()))?;
        if !entry
            .file_type()
            .map_err(|error| format!("could not inspect `{}`: {error}", entry.path().display()))?
            .is_dir()
        {
            continue;
        }
        let path = entry.path();
        let repositories = fs::read_dir(&path)
            .map_err(|error| format!("could not read `{}`: {error}", path.display()))?
            .filter_map(std::result::Result::ok)
            .filter(|child| child.path().is_dir())
            .count();
        entries.push(ProjectEntry {
            name: entry.file_name().to_string_lossy().into_owned(),
            path,
            repositories,
        });
    }
    entries.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(entries)
}

fn workspace_root() -> Result<PathBuf> {
    if let Some(configured) = env::var_os(WORKSPACE_ENV_VAR).filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(configured));
    }
    Ok(home_directory()?.join(WORKSPACE_DIRECTORY))
}

fn trash_directory() -> Result<PathBuf> {
    if let Some(configured) = env::var_os(TRASH_ENV_VAR).filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(configured));
    }
    if cfg!(target_os = "macos") {
        Ok(home_directory()?.join(".Trash"))
    } else {
        Ok(home_directory()?.join(".local/share/Trash/files"))
    }
}

fn home_directory() -> Result<PathBuf> {
    env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| {
            format!("could not find the home directory; set `HOME` or `{WORKSPACE_ENV_VAR}`.")
        })
}

fn take_flag(args: &mut Vec<String>, flag: &str) -> bool {
    if let Some(index) = args.iter().position(|argument| argument == flag) {
        args.remove(index);
        true
    } else {
        false
    }
}

fn shell_quote(path: &str) -> String {
    format!("'{}'", path.replace('\'', "'\\''"))
}

#[derive(Clone, Copy)]
enum Destination {
    Stdout,
    Stderr,
    #[cfg(test)]
    Silent,
}

struct Printer {
    destination: Destination,
    theme: Theme,
}

impl Printer {
    fn new(destination: Destination) -> Self {
        let theme = match destination {
            Destination::Stdout => Theme::stdout(),
            Destination::Stderr => Theme::stderr(),
            #[cfg(test)]
            Destination::Silent => Theme { enabled: false },
        };
        Self { destination, theme }
    }

    fn status(&self, message: String) {
        match self.destination {
            Destination::Stdout => println!("{message}"),
            Destination::Stderr => eprintln!("{message}"),
            #[cfg(test)]
            Destination::Silent => {}
        }
    }
}

#[derive(Clone, Copy)]
struct Theme {
    enabled: bool,
}

impl Theme {
    fn stdout() -> Self {
        Self {
            enabled: env::var_os("NO_COLOR").is_none() && io::stdout().is_terminal(),
        }
    }

    fn stderr() -> Self {
        Self {
            enabled: env::var_os("NO_COLOR").is_none() && io::stderr().is_terminal(),
        }
    }

    fn blue(&self, value: &str) -> String {
        self.paint(BLUE, value)
    }

    fn pink(&self, value: &str) -> String {
        self.paint(PINK, value)
    }

    fn yellow(&self, value: &str) -> String {
        self.paint(YELLOW, value)
    }

    fn paint(&self, color: &str, value: &str) -> String {
        if self.enabled {
            format!("\u{1b}[38;2;{color}m{value}\u{1b}[0m")
        } else {
            value.to_string()
        }
    }

    fn banner_letter(&self, value: &str, color: &str) -> String {
        self.paint(color, value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git_env() -> [(&'static str, &'static str); 4] {
        [
            ("GIT_AUTHOR_NAME", "aicasa"),
            ("GIT_AUTHOR_EMAIL", "aicasa@example.com"),
            ("GIT_COMMITTER_NAME", "aicasa"),
            ("GIT_COMMITTER_EMAIL", "aicasa@example.com"),
        ]
    }

    fn temporary_directory(name: &str) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = env::temp_dir().join(format!(
            "{PROGRAM_NAME}-{name}-{}-{timestamp}",
            process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn local_bare_repository(parent: &Path, name: &str) -> Repository {
        let source = parent.join(format!("{name}.git"));
        assert!(
            Command::new("git")
                .arg("init")
                .arg("--bare")
                .arg("-q")
                .arg(&source)
                .status()
                .unwrap()
                .success()
        );
        Repository {
            source: source.display().to_string(),
            directory: name.to_string(),
        }
    }

    fn local_seeded_bare_repository(parent: &Path, name: &str) -> Repository {
        let source = parent.join(format!("{name}.git"));
        let working = parent.join(format!("{name}-working"));
        assert!(
            Command::new("git")
                .arg("init")
                .arg("-q")
                .arg(&working)
                .status()
                .unwrap()
                .success()
        );
        fs::write(working.join("README.md"), format!("#{name}\n")).unwrap();
        let mut add = Command::new("git");
        add.arg("-C").arg(&working).arg("add").arg("README.md");
        for (key, value) in git_env() {
            add.env(key, value);
        }
        assert!(add.status().unwrap().success());
        let mut commit = Command::new("git");
        commit
            .arg("-C")
            .arg(&working)
            .arg("commit")
            .arg("-q")
            .arg("-m")
            .arg("initial commit");
        for (key, value) in git_env() {
            commit.env(key, value);
        }
        assert!(commit.status().unwrap().success());
        assert!(
            Command::new("git")
                .arg("init")
                .arg("--bare")
                .arg("-q")
                .arg(&source)
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(&working)
                .arg("remote")
                .arg("add")
                .arg("origin")
                .arg(&source)
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(&working)
                .arg("push")
                .arg("-u")
                .arg("origin")
                .arg("HEAD")
                .status()
                .unwrap()
                .success()
        );
        Repository {
            source: source.display().to_string(),
            directory: name.to_string(),
        }
    }

    fn current_branch_name(repository_path: &Path) -> Result<Option<String>> {
        let output = Command::new("git")
            .arg("-C")
            .arg(repository_path)
            .arg("branch")
            .arg("--show-current")
            .output()
            .map_err(|error| format!("could not read current branch: {error}"))?;
        if !output.status.success() {
            return Err("git could not determine the current branch.".to_string());
        }
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if branch.is_empty() {
            Ok(None)
        } else {
            Ok(Some(branch))
        }
    }

    #[test]
    fn parses_comma_separated_github_repositories() {
        let repositories =
            parse_repositories(&["paradise-runner/toast,paradise-runner/kaleidoscope".into()])
                .unwrap();
        assert_eq!(
            repositories,
            vec![
                Repository {
                    source: "https://github.com/paradise-runner/toast.git".into(),
                    directory: "toast".into()
                },
                Repository {
                    source: "https://github.com/paradise-runner/kaleidoscope.git".into(),
                    directory: "kaleidoscope".into()
                }
            ]
        );
    }

    #[test]
    fn refuses_nested_project_names() {
        assert!(validate_project_name("../elsewhere").is_err());
        assert!(validate_project_name("nested/project").is_err());
        assert!(validate_project_name("project").is_ok());
    }

    #[test]
    fn renders_a_plain_aicasa_banner_without_escape_codes() {
        let banner = render_banner(&Theme { enabled: false });

        assert!(banner.starts_with(" █████╗ ██╗ ██████╗"));
        assert_eq!(banner.lines().count(), 6);
        assert!(!banner.contains('\u{1b}'));
    }

    #[test]
    fn renders_a_pastel_aicasa_banner_when_color_is_enabled() {
        let banner = render_banner(&Theme { enabled: true });

        assert!(banner.contains(&format!("\u{1b}[38;2;{BLUE}m")));
        assert!(banner.contains(&format!("\u{1b}[38;2;{PINK}m")));
        assert!(banner.contains(&format!("\u{1b}[38;2;{YELLOW}m")));
    }

    #[test]
    fn creates_adds_lists_and_trashes_a_workspace() {
        let temporary = temporary_directory("lifecycle");
        let root = temporary.join("workspaces");
        let trash = temporary.join("trash");
        let sources = temporary.join("sources");
        fs::create_dir_all(&sources).unwrap();
        let first = local_seeded_bare_repository(&sources, "toast");
        let second = local_bare_repository(&sources, "kaleidoscope");
        let printer = Printer::new(Destination::Silent);

        let project = create_project(&root, "demo", &[first], &printer).unwrap();
        assert!(project.join("toast/.git").is_dir());
        assert_eq!(
            current_branch_name(&project.join("toast")).unwrap(),
            Some("demo".into())
        );
        add_repositories(&project, &[second], &printer).unwrap();
        assert!(project.join("kaleidoscope/.git").is_dir());
        assert_ne!(
            current_branch_name(&project.join("kaleidoscope")).unwrap(),
            Some("demo".into())
        );
        let metadata = read_workspace_metadata(&project).unwrap().unwrap();
        assert_eq!(metadata.schema_version, METADATA_SCHEMA_VERSION);
        assert_eq!(metadata.name, "demo");
        assert_eq!(
            metadata.repositories,
            vec![
                StoredRepository {
                    source: Some(sources.join("toast.git").display().to_string()),
                    directory: "toast".into()
                },
                StoredRepository {
                    source: Some(sources.join("kaleidoscope.git").display().to_string()),
                    directory: "kaleidoscope".into()
                }
            ]
        );
        let inspection = serde_json::to_value(inspect_workspace(&project).unwrap()).unwrap();
        assert_eq!(inspection["metadata_present"], true);
        assert_eq!(inspection["repositories"][0]["directory"], "toast");
        assert_eq!(inspection["repositories"][0]["exists"], true);

        fs::remove_dir_all(project.join("toast")).unwrap();
        add_repositories(
            &project,
            &[Repository {
                source: sources.join("toast.git").display().to_string(),
                directory: "toast".into(),
            }],
            &printer,
        )
        .unwrap();
        assert!(project.join("toast/.git").is_dir());
        assert_eq!(
            read_workspace_metadata(&project)
                .unwrap()
                .unwrap()
                .repositories
                .len(),
            2
        );

        assert_eq!(
            list_projects(&root).unwrap(),
            vec![ProjectEntry {
                name: "demo".into(),
                path: project.clone(),
                repositories: 2
            }]
        );

        move_to_trash(&root, &trash, &["demo".to_string()], &printer).unwrap();
        assert!(!project.exists());
        assert!(trash.join("demo/toast/.git").is_dir());
        fs::remove_dir_all(&temporary).unwrap();
    }

    #[test]
    fn creates_project_branch_even_for_empty_remote_repositories() {
        let temporary = temporary_directory("empty-remote-branch");
        let root = temporary.join("workspaces");
        let sources = temporary.join("sources");
        fs::create_dir_all(&sources).unwrap();
        let empty = local_bare_repository(&sources, "toast");

        let project =
            create_project(&root, "demo", &[empty], &Printer::new(Destination::Silent)).unwrap();

        assert_eq!(
            current_branch_name(&project.join("toast")).unwrap(),
            Some("demo".into())
        );

        fs::remove_dir_all(&temporary).unwrap();
    }

    #[test]
    fn inspects_legacy_workspaces_without_creating_metadata() {
        let temporary = temporary_directory("legacy-inspect");
        let project = temporary.join("legacy");
        let sources = temporary.join("sources");
        fs::create_dir_all(project.join("existing-repo")).unwrap();
        fs::create_dir_all(&sources).unwrap();

        let inspection = serde_json::to_value(inspect_workspace(&project).unwrap()).unwrap();
        assert_eq!(inspection["metadata_present"], false);
        assert_eq!(inspection["repositories"][0]["directory"], "existing-repo");
        assert_eq!(
            inspection["repositories"][0]["source"],
            serde_json::Value::Null
        );
        assert!(!project.join(METADATA_FILE).exists());

        let new_repository = local_bare_repository(&sources, "new-repo");
        add_repositories(
            &project,
            &[new_repository],
            &Printer::new(Destination::Silent),
        )
        .unwrap();
        let metadata = read_workspace_metadata(&project).unwrap().unwrap();
        assert_eq!(metadata.repositories[0].directory, "existing-repo");
        assert_eq!(metadata.repositories[0].source, None);
        assert_eq!(metadata.repositories[1].directory, "new-repo");
        assert_eq!(
            metadata.repositories[1].source,
            Some(sources.join("new-repo.git").display().to_string())
        );
        fs::remove_dir_all(&temporary).unwrap();
    }

    #[test]
    fn uses_a_unique_trash_name_when_a_name_is_already_present() {
        let temporary = temporary_directory("trash-name");
        fs::create_dir(temporary.join("demo")).unwrap();
        let destination = unused_trash_path(&temporary, "demo");
        assert_ne!(destination, temporary.join("demo"));
        assert!(
            destination
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("demo-")
        );
        fs::remove_dir_all(&temporary).unwrap();
    }

    #[test]
    fn refuses_to_trash_a_workspace_twice_in_one_command() {
        let temporary = temporary_directory("duplicate-rm");
        let root = temporary.join("workspaces");
        let trash = temporary.join("trash");
        fs::create_dir_all(root.join("demo")).unwrap();
        let printer = Printer::new(Destination::Silent);

        assert!(
            move_to_trash(
                &root,
                &trash,
                &["demo".to_string(), "demo".to_string()],
                &printer
            )
            .is_err()
        );
        assert!(root.join("demo").is_dir());
        fs::remove_dir_all(&temporary).unwrap();
    }
}
