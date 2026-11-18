use std::collections::{HashMap, HashSet};
use std::process::Command;

#[derive(Clone, Debug)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub origin: String,
    pub comment: String,
}

// Pacotes críticos do sistema que NUNCA podem ser removidos
const CRITICAL_PACKAGES: &[&str] = &["pkg", "AppStream", "xorg", "xfce4", "libadwaita", "gtk4"];

pub fn is_critical_package(name: &str) -> bool {
    CRITICAL_PACKAGES.iter().any(|&p| p.eq_ignore_ascii_case(name))
}

#[derive(Clone, Debug, Default)]
pub struct PackageDetails {
    pub name: String,
    pub version: String,
    pub origin: String,
    pub comment: String,
    pub www: String,
    pub maintainer: String,
    pub arch: String,
    pub size: String,
    pub description: String,
    pub categories: Vec<String>,
    pub licenses: Vec<String>,
    /// (nome, versão)
    pub depends: Vec<(String, String)>,
    pub required_by: Vec<(String, String)>,
    pub installed: bool,
}

const FMT: &str = "%n\t%v\t%o\t%c";

fn parse_pkg_output(output: Result<std::process::Output, std::io::Error>) -> Vec<Package> {
    let mut packages = Vec::new();
    if let Ok(out) = output {
        if out.status.success() {
            let stdout = String::from_utf8_lossy(&out.stdout);
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() >= 3 {
                    packages.push(Package {
                        name: parts[0].to_string(),
                        version: parts[1].to_string(),
                        origin: parts[2].to_string(),
                        comment: parts.get(3).map(|s| s.to_string()).unwrap_or_default(),
                    });
                }
            }
        }
    }
    packages
}

pub fn list_installed_packages() -> Vec<Package> {
    parse_pkg_output(Command::new("pkg").arg("query").arg(FMT).output())
}

/// Apenas os nomes dos pacotes instalados (rápido, usado para marcar "instalado").
pub fn list_installed_names() -> HashSet<String> {
    match Command::new("pkg").arg("query").arg("%n").output() {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect(),
        _ => HashSet::new(),
    }
}

/// Todos os pacotes dos repositórios (catálogo local). Carregado UMA vez e
/// filtrado em memória à medida que o utilizador escreve.
pub fn list_available_packages() -> Vec<Package> {
    parse_pkg_output(Command::new("pkg").arg("rquery").arg("-U").arg(FMT).output())
}

/// Atualizações disponíveis. `pkg upgrade -n` não aceita formato de saída,
/// por isso lê-se o resumo da simulação (dry-run).
pub fn list_upgrades() -> Vec<Package> {
    let output = Command::new("pkg")
        .env("LC_ALL", "C")
        .args(["upgrade", "-n", "-U"])
        .output();
    let out = match output {
        Ok(o) => o,
        Err(_) => return vec![],
    };

    let installed: HashMap<String, Package> = list_installed_packages()
        .into_iter()
        .map(|p| (p.name.clone(), p))
        .collect();

    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut in_section = false;
    let mut result = Vec::new();

    for line in stdout.lines() {
        if line.contains("to be UPGRADED") {
            in_section = true;
            continue;
        }
        if !line.starts_with(char::is_whitespace) {
            in_section = false;
            continue;
        }
        if !in_section || line.trim().is_empty() {
            continue;
        }
        // Formato: "\tnome: 1.0 -> 1.1 [Repo]"
        if let Some((name, rest)) = line.trim().split_once(": ") {
            let version = rest.split(" [").next().unwrap_or(rest).trim().to_string();
            let (origin, comment) = installed
                .get(name)
                .map(|p| (p.origin.clone(), p.comment.clone()))
                .unwrap_or_default();
            result.push(Package {
                name: name.to_string(),
                version,
                origin,
                comment,
            });
        }
    }
    result
}

/// Órfãos = instalados automaticamente (como dependência, %a == 1) e que já
/// nenhum outro pacote necessita (%#r == 0). É o mesmo critério do `pkg autoremove`.
pub fn list_orphan_packages() -> Vec<Package> {
    let output = Command::new("pkg")
        .arg("query")
        .arg("-e")
        .arg("%a == 1 && %#r == 0")
        .arg(FMT)
        .output();
    parse_pkg_output(output)
        .into_iter()
        .filter(|p| !is_critical_package(&p.name))
        .collect()
}

fn run_pkexec(args: &[&str]) -> bool {
    Command::new("pkexec")
        .arg("pkg")
        .args(args)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn refresh_repositories() -> bool {
    run_pkexec(&["update"])
}

pub fn upgrade_all_packages() -> bool {
    run_pkexec(&["upgrade", "-y"])
}

pub fn install_package(name: &str) -> bool {
    run_pkexec(&["install", "-y", name])
}

pub fn remove_package(name: &str) -> bool {
    if is_critical_package(name) {
        return false;
    }
    run_pkexec(&["delete", "-y", name])
}

/// Remove vários pacotes de uma só vez (um único pedido de palavra-passe).
pub fn remove_packages(names: &[String]) -> bool {
    let safe: Vec<&str> = names
        .iter()
        .map(|n| n.as_str())
        .filter(|n| !is_critical_package(n))
        .collect();
    if safe.is_empty() {
        return false;
    }
    let mut args = vec!["delete", "-y"];
    args.extend(safe);
    run_pkexec(&args)
}

pub fn reinstall_package(name: &str) -> bool {
    run_pkexec(&["install", "-f", "-y", name])
}

// ---------------------------------------------------------------------------
// Detalhes de um pacote
// ---------------------------------------------------------------------------

fn query_lines(local: bool, fmt: &str, name: &str) -> Vec<String> {
    let mut cmd = Command::new("pkg");
    if local {
        cmd.arg("query");
    } else {
        cmd.arg("rquery").arg("-U");
    }
    cmd.arg(fmt).arg(name);
    match cmd.output() {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(|l| l.to_string())
            .collect(),
        _ => vec![],
    }
}

fn pairs(lines: Vec<String>) -> Vec<(String, String)> {
    lines
        .into_iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let mut it = l.splitn(2, '\t');
            let n = it.next().unwrap_or("").trim().to_string();
            let v = it.next().unwrap_or("").trim().to_string();
            (n, v)
        })
        .collect()
}

fn non_empty(lines: Vec<String>) -> Vec<String> {
    lines
        .into_iter()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Especificações completas de um pacote (instalado: base local;
/// caso contrário: catálogo dos repositórios).
pub fn package_details(name: &str, installed: bool) -> PackageDetails {
    const SCALAR: &str = "%n\t%v\t%o\t%c\t%w\t%m\t%q\t%sh";

    let mut local = installed;
    let mut scalar = query_lines(local, SCALAR, name);
    if scalar.is_empty() && local {
        local = false;
        scalar = query_lines(false, SCALAR, name);
    }

    let mut d = PackageDetails {
        name: name.to_string(),
        installed,
        ..Default::default()
    };

    if let Some(line) = scalar.first() {
        let p: Vec<&str> = line.split('\t').collect();
        let g = |i: usize| p.get(i).map(|s| s.trim().to_string()).unwrap_or_default();
        if !g(0).is_empty() {
            d.name = g(0);
        }
        d.version = g(1);
        d.origin = g(2);
        d.comment = g(3);
        d.www = g(4);
        d.maintainer = g(5);
        d.arch = g(6);
        d.size = g(7);
    }

    d.description = query_lines(local, "%e", name).join("\n").trim().to_string();
    d.categories = non_empty(query_lines(local, "%C", name));
    d.licenses = non_empty(query_lines(local, "%L", name));
    d.depends = pairs(query_lines(local, "%dn\t%dv", name));
    d.required_by = pairs(query_lines(local, "%rn\t%rv", name));
    d
}