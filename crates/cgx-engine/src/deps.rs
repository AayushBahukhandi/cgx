use std::path::Path;

use anyhow::Context;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ecosystem {
    Npm,
    Cargo,
    PyPI,
    Go,
}

impl Ecosystem {
    pub fn as_str(&self) -> &'static str {
        match self {
            Ecosystem::Npm => "npm",
            Ecosystem::Cargo => "crates.io",
            Ecosystem::PyPI => "PyPI",
            Ecosystem::Go => "Go",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub name: String,
    pub version: String,
    pub ecosystem: Ecosystem,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyReport {
    pub name: String,
    pub version: String,
    pub ecosystem: String,
    pub cve_count: i64,
    pub cve_ids: Vec<String>,
    pub risk_score: f64,
}

pub fn parse_manifests(repo_root: &Path) -> anyhow::Result<Vec<Dependency>> {
    let mut deps: Vec<Dependency> = Vec::new();

    // package.json — npm
    let pkg_json = repo_root.join("package.json");
    if pkg_json.exists() {
        if let Ok(content) = std::fs::read_to_string(&pkg_json) {
            deps.extend(parse_package_json(&content));
        }
    }

    // Cargo.toml — Rust
    let cargo_toml = repo_root.join("Cargo.toml");
    if cargo_toml.exists() {
        if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
            deps.extend(parse_cargo_toml(&content));
        }
    }

    // requirements.txt — Python
    let reqs_txt = repo_root.join("requirements.txt");
    if reqs_txt.exists() {
        if let Ok(content) = std::fs::read_to_string(&reqs_txt) {
            deps.extend(parse_requirements_txt(&content));
        }
    }

    // pyproject.toml — Python
    let pyproject = repo_root.join("pyproject.toml");
    if pyproject.exists() {
        if let Ok(content) = std::fs::read_to_string(&pyproject) {
            deps.extend(parse_pyproject_toml(&content));
        }
    }

    // go.mod — Go
    let go_mod = repo_root.join("go.mod");
    if go_mod.exists() {
        if let Ok(content) = std::fs::read_to_string(&go_mod) {
            deps.extend(parse_go_mod(&content));
        }
    }

    Ok(deps)
}

fn parse_package_json(content: &str) -> Vec<Dependency> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(content) else {
        return Vec::new();
    };
    let mut deps = Vec::new();
    for section in &["dependencies", "devDependencies"] {
        if let Some(map) = v.get(section).and_then(|v| v.as_object()) {
            for (name, ver) in map {
                let version = ver
                    .as_str()
                    .unwrap_or("*")
                    .trim_start_matches('^')
                    .trim_start_matches('~')
                    .trim_start_matches('>')
                    .trim_start_matches('=')
                    .to_string();
                deps.push(Dependency {
                    name: name.clone(),
                    version,
                    ecosystem: Ecosystem::Npm,
                });
            }
        }
    }
    deps
}

fn parse_cargo_toml(content: &str) -> Vec<Dependency> {
    let Ok(v) = content.parse::<toml::Value>() else {
        return Vec::new();
    };
    let mut deps = Vec::new();
    if let Some(table) = v.get("dependencies").and_then(|v| v.as_table()) {
        for (name, ver) in table {
            let version = match ver {
                toml::Value::String(s) => s.trim_start_matches('^').to_string(),
                toml::Value::Table(t) => t
                    .get("version")
                    .and_then(|v| v.as_str())
                    .unwrap_or("*")
                    .trim_start_matches('^')
                    .to_string(),
                _ => "*".to_string(),
            };
            deps.push(Dependency {
                name: name.clone(),
                version,
                ecosystem: Ecosystem::Cargo,
            });
        }
    }
    deps
}

fn parse_requirements_txt(content: &str) -> Vec<Dependency> {
    let mut deps = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('-') {
            continue;
        }
        let (name, version) = if let Some(pos) = line.find("==") {
            (&line[..pos], line[pos + 2..].to_string())
        } else if let Some(pos) = line.find(">=") {
            (&line[..pos], line[pos + 2..].to_string())
        } else {
            (line, "*".to_string())
        };
        deps.push(Dependency {
            name: name.trim().to_string(),
            version: version.trim().to_string(),
            ecosystem: Ecosystem::PyPI,
        });
    }
    deps
}

fn parse_pyproject_toml(content: &str) -> Vec<Dependency> {
    let Ok(v) = content.parse::<toml::Value>() else {
        return Vec::new();
    };
    let mut deps = Vec::new();
    // PEP 621 style
    if let Some(arr) = v
        .get("project")
        .and_then(|p| p.get("dependencies"))
        .and_then(|d| d.as_array())
    {
        for dep in arr {
            if let Some(s) = dep.as_str() {
                let (name, version) = parse_pep508(s);
                deps.push(Dependency {
                    name,
                    version,
                    ecosystem: Ecosystem::PyPI,
                });
            }
        }
    }
    deps
}

fn parse_pep508(spec: &str) -> (String, String) {
    if let Some(pos) = spec.find(">=") {
        (
            spec[..pos].trim().to_string(),
            spec[pos + 2..].trim().to_string(),
        )
    } else if let Some(pos) = spec.find("==") {
        (
            spec[..pos].trim().to_string(),
            spec[pos + 2..].trim().to_string(),
        )
    } else {
        (spec.trim().to_string(), "*".to_string())
    }
}

fn parse_go_mod(content: &str) -> Vec<Dependency> {
    let mut deps = Vec::new();
    let mut in_require = false;
    for line in content.lines() {
        let line = line.trim();
        if line == "require (" {
            in_require = true;
            continue;
        }
        if line == ")" {
            in_require = false;
            continue;
        }
        if line.starts_with("require ") {
            // single-line require
            let rest = line.trim_start_matches("require ").trim();
            if let Some((name, ver)) = rest.split_once(' ') {
                deps.push(Dependency {
                    name: name.trim().to_string(),
                    version: ver.trim().trim_start_matches('v').to_string(),
                    ecosystem: Ecosystem::Go,
                });
            }
            continue;
        }
        if in_require && !line.is_empty() && !line.starts_with("//") {
            if let Some((name, ver)) = line.split_once(' ') {
                let ver_clean = ver.trim().trim_start_matches('v');
                if !ver_clean.contains("//") {
                    deps.push(Dependency {
                        name: name.trim().to_string(),
                        version: ver_clean.trim().to_string(),
                        ecosystem: Ecosystem::Go,
                    });
                }
            }
        }
    }
    deps
}

/// Query OSV for vulnerabilities. Falls back gracefully on network failure.
pub fn query_osv(deps: &[Dependency]) -> Vec<DependencyReport> {
    let queries: Vec<serde_json::Value> = deps
        .iter()
        .map(|d| {
            serde_json::json!({
                "package": {
                    "name": d.name,
                    "ecosystem": d.ecosystem.as_str()
                },
                "version": d.version
            })
        })
        .collect();

    let client = match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(_) => return build_reports_no_cve(deps),
    };

    let body = serde_json::json!({ "queries": queries });
    let resp = client
        .post("https://api.osv.dev/v1/querybatch")
        .json(&body)
        .send();

    match resp {
        Err(_) => {
            eprintln!("  Warning: CVE data unavailable (network error)");
            build_reports_no_cve(deps)
        }
        Ok(r) => {
            let Ok(json) = r.json::<serde_json::Value>() else {
                eprintln!("  Warning: CVE data unavailable (parse error)");
                return build_reports_no_cve(deps);
            };

            let results = json
                .get("results")
                .and_then(|r| r.as_array())
                .map(|a| a.as_slice())
                .unwrap_or(&[]);

            deps.iter()
                .enumerate()
                .map(|(i, dep)| {
                    let vulns = results
                        .get(i)
                        .and_then(|r| r.get("vulns"))
                        .and_then(|v| v.as_array());

                    let cve_ids: Vec<String> = vulns
                        .map(|v| {
                            v.iter()
                                .filter_map(|vuln| {
                                    vuln.get("id")
                                        .and_then(|id| id.as_str())
                                        .map(|s| s.to_string())
                                })
                                .collect()
                        })
                        .unwrap_or_default();

                    let cve_count = cve_ids.len() as i64;
                    let risk_score = if cve_count > 0 {
                        (cve_count as f64).min(5.0) / 5.0
                    } else {
                        0.0
                    };

                    DependencyReport {
                        name: dep.name.clone(),
                        version: dep.version.clone(),
                        ecosystem: dep.ecosystem.as_str().to_string(),
                        cve_count,
                        cve_ids,
                        risk_score,
                    }
                })
                .collect()
        }
    }
}

fn build_reports_no_cve(deps: &[Dependency]) -> Vec<DependencyReport> {
    deps.iter()
        .map(|d| DependencyReport {
            name: d.name.clone(),
            version: d.version.clone(),
            ecosystem: d.ecosystem.as_str().to_string(),
            cve_count: 0,
            cve_ids: Vec::new(),
            risk_score: 0.0,
        })
        .collect()
}

/// Parse manifests and query OSV — returns reports, network failures handled gracefully.
pub fn audit_dependencies(repo_root: &Path) -> anyhow::Result<Vec<DependencyReport>> {
    let deps = parse_manifests(repo_root).context("Failed to parse manifests")?;
    if deps.is_empty() {
        return Ok(Vec::new());
    }
    Ok(query_osv(&deps))
}
