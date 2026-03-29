use super::{NamingDefinition, Extractor};

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "toml",
        extension: "toml",
        extract: Extractor::Custom(|content| crate::structured::extract_toml(content)),
    }
}

#[cfg(test)]
mod tests {
    use crate::suggest_stem;

    fn name(src: &str) -> Option<String> {
        suggest_stem(src, "toml")
    }

    #[test]
    fn toml_cargo_package() {
        let src = "[package]\nname = \"grayslate\"\nversion = \"0.1.0\"\nedition = \"2021\"";
        let n = name(src).unwrap();
        assert!(n.contains("grayslate"), "Cargo package name: {n}");
    }

    #[test]
    fn toml_pyproject() {
        let src = "[project]\nname = \"data-pipeline\"\nversion = \"2.0.0\"\n\n[build-system]\nrequires = [\"setuptools\"]";
        let n = name(src).unwrap();
        assert!(n.contains("data-pipeline"), "pyproject name: {n}");
    }
}
