//! GIFs que o usuário importou para usar como bichinho. Ficam só na pasta de dados do app.

use std::path::{Path, PathBuf};

/// Nome de exibição = nome do arquivo sem `.gif`. Recusa qualquer coisa que pareça caminho.
fn file_for(dir: &Path, name: &str) -> Result<PathBuf, String> {
    let valid = !name.is_empty()
        && !name.contains(['/', '\\', ':'])
        && name != "."
        && name != ".."
        && !name.starts_with('.');
    if !valid {
        return Err(format!("Nome de bichinho inválido: {name}"));
    }
    Ok(dir.join(format!("{name}.gif")))
}

pub fn list(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("gif")))
        .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().to_string()))
        .collect();
    names.sort_by_key(|n| n.to_lowercase());
    names
}

pub fn read(dir: &Path, name: &str) -> Result<Vec<u8>, String> {
    std::fs::read(file_for(dir, name)?).map_err(|e| format!("Não foi possível ler {name}: {e}"))
}

/// Copia o GIF para a pasta do app e devolve o nome com que ficou salvo.
pub fn add(dir: &Path, source: &Path) -> Result<String, String> {
    let bytes =
        std::fs::read(source).map_err(|e| format!("Não foi possível abrir o arquivo: {e}"))?;
    if !bytes.starts_with(b"GIF8") {
        return Err("Esse arquivo não é um GIF.".into());
    }
    let stem = source
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let base: String = stem
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || " -_".contains(c) {
                c
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(40)
        .collect();
    let base = if base.is_empty() {
        "Bichinho".to_string()
    } else {
        base
    };

    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut name = base.clone();
    let mut n = 2;
    while file_for(dir, &name)?.exists() {
        name = format!("{base} {n}");
        n += 1;
    }
    std::fs::write(file_for(dir, &name)?, bytes)
        .map_err(|e| format!("Não foi possível salvar o GIF: {e}"))?;
    Ok(name)
}

pub fn remove(dir: &Path, name: &str) -> Result<(), String> {
    std::fs::remove_file(file_for(dir, name)?)
        .map_err(|e| format!("Não foi possível remover {name}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const GIF: &[u8] = b"GIF89a\x01\x00\x01\x00";

    #[test]
    fn add_list_read_remove() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("pets");
        let src = tmp.path().join("⚡️ Pikachu ⚡️ ~ [GiF].gif");
        std::fs::write(&src, GIF).unwrap();

        assert_eq!(add(&dir, &src).unwrap(), "Pikachu GiF");
        assert_eq!(add(&dir, &src).unwrap(), "Pikachu GiF 2");
        assert_eq!(list(&dir), vec!["Pikachu GiF", "Pikachu GiF 2"]);
        assert_eq!(read(&dir, "Pikachu GiF").unwrap(), GIF);
        remove(&dir, "Pikachu GiF 2").unwrap();
        assert_eq!(list(&dir), vec!["Pikachu GiF"]);
    }

    #[test]
    fn rejects_non_gif_and_path_names() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("foto.gif");
        std::fs::write(&src, b"\x89PNG").unwrap();
        assert!(add(tmp.path(), &src).is_err());
        assert!(read(tmp.path(), "../segredo").is_err());
        assert!(read(tmp.path(), "C:\\Windows\\x").is_err());
        assert!(remove(tmp.path(), "..").is_err());
    }

    #[test]
    fn missing_dir_lists_nothing() {
        assert!(list(Path::new("Z:\\nao-existe")).is_empty());
    }
}
