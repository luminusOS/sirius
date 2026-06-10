//! Minimal in-app translation. `Lang` is the UI language; `tr` looks up a key.
//! Falls back to English (and then to the key itself) so missing keys are never blank.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    PtBr,
}

impl Lang {
    /// Map an install locale string (e.g. "pt_BR", "en_US") to a UI language.
    pub fn from_locale(locale: &str) -> Lang {
        if locale.to_ascii_lowercase().starts_with("pt") {
            Lang::PtBr
        } else {
            Lang::En
        }
    }
}

impl Default for Lang {
    fn default() -> Self {
        Lang::En
    }
}

/// Translate a key for the given language. English is the base; pt_BR overrides.
pub fn tr(lang: Lang, key: &str) -> &'static str {
    // English base table — every key MUST have an entry here.
    let en = |k: &str| -> &'static str {
        match k {
            "nav.back" => "Back",
            "nav.next" => "Next",
            "welcome.title" => "Welcome",
            "welcome.desc" => "This assistant will guide you through installation.",
            "network.title" => "Network",
            "network.desc" => "Connect to a network. A connection is optional but recommended.",
            "network.body" => "Use the system network indicator to connect.",
            "keyboard.title" => "Keyboard layout",
            "keyboard.test" => "Type here to test your layout",
            "timezone.title" => "Time zone",
            "partition.title" => "Partitioning",
            "partition.group" => "Automatic",
            "partition.encrypt" => "Encrypt the disk (LUKS)",
            "partition.tpm" => "Bind encryption to TPM",
            "user.title" => "Create your account",
            "user.full_name" => "Full name",
            "user.username" => "Username",
            "user.password" => "Password",
            "user.confirm" => "Confirm password",
            "user.hostname" => "Hostname",
            "summary.title" => "Ready to install",
            "summary.desc" => "Review your choices. The disk will be erased.",
            "summary.language" => "Language",
            "summary.keyboard" => "Keyboard",
            "summary.timezone" => "Time zone",
            "summary.disk" => "Disk",
            "summary.encryption" => "Encryption",
            "summary.user" => "User",
            "summary.hostname" => "Hostname",
            "summary.enabled" => "enabled",
            "summary.disabled" => "disabled",
            "progress.title" => "Installing the system",
            "finished.title" => "Installation complete",
            "finished.desc" => "The system is installed. Reboot to start using it.",
            "finished.restart" => "Restart now",
            _ => "",
        }
    };
    // Portuguese overrides — only keys that are translated.
    let pt = |k: &str| -> &'static str {
        match k {
            "nav.back" => "Voltar",
            "nav.next" => "Avançar",
            "welcome.title" => "Bem-vindo",
            "welcome.desc" => "Este assistente vai guiá-lo pela instalação.",
            "network.title" => "Rede",
            "network.desc" => "Conecte-se a uma rede. A conexão é opcional, mas recomendada.",
            "network.body" => "Use o indicador de rede do sistema para conectar.",
            "keyboard.title" => "Layout do teclado",
            "keyboard.test" => "Digite aqui para testar o layout",
            "timezone.title" => "Fuso horário",
            "partition.title" => "Particionamento",
            "partition.group" => "Automático",
            "partition.encrypt" => "Criptografar o disco (LUKS)",
            "partition.tpm" => "Vincular criptografia ao TPM",
            "user.title" => "Crie sua conta",
            "user.full_name" => "Nome completo",
            "user.username" => "Nome de usuário",
            "user.password" => "Senha",
            "user.confirm" => "Confirmar senha",
            "user.hostname" => "Nome da máquina",
            "summary.title" => "Pronto para instalar",
            "summary.desc" => "Revise suas escolhas. O disco será apagado.",
            "summary.language" => "Idioma",
            "summary.keyboard" => "Teclado",
            "summary.timezone" => "Fuso horário",
            "summary.disk" => "Disco",
            "summary.encryption" => "Criptografia",
            "summary.user" => "Usuário",
            "summary.hostname" => "Nome da máquina",
            "summary.enabled" => "ativada",
            "summary.disabled" => "desativada",
            "progress.title" => "Instalando o sistema",
            "finished.title" => "Instalação concluída",
            "finished.desc" => "O sistema foi instalado. Reinicie para começar a usá-lo.",
            "finished.restart" => "Reiniciar agora",
            _ => "",
        }
    };
    match lang {
        Lang::PtBr => {
            let p = pt(key);
            if !p.is_empty() { p } else { let e = en(key); if e.is_empty() { leak_key(key) } else { e } }
        }
        Lang::En => {
            let e = en(key);
            if e.is_empty() { leak_key(key) } else { e }
        }
    }
}

/// Last-resort fallback: show the key itself (leaked to 'static) so nothing is blank.
fn leak_key(key: &str) -> &'static str {
    Box::leak(key.to_string().into_boxed_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_locale_maps_pt() {
        assert_eq!(Lang::from_locale("pt_BR"), Lang::PtBr);
        assert_eq!(Lang::from_locale("en_US"), Lang::En);
    }

    #[test]
    fn tr_translates_and_falls_back() {
        assert_eq!(tr(Lang::PtBr, "nav.next"), "Avançar");
        assert_eq!(tr(Lang::En, "nav.next"), "Next");
        // Unknown key falls back to the key text, never blank.
        assert_eq!(tr(Lang::En, "unknown.key"), "unknown.key");
    }
}
