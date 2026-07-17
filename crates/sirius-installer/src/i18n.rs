//! Minimal in-app translation. `Lang` is the UI language; `tr` looks up a key.
//! Falls back to English (and then to the key itself) so missing keys are never blank.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    #[default]
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

/// Translate a key for the given language. English is the base; pt_BR overrides.
pub fn tr(lang: Lang, key: &str) -> &'static str {
    // English base table — every key MUST have an entry here.
    let en = |k: &str| -> &'static str {
        match k {
            "nav.back" => "Back",
            "nav.next" => "Next",
            "nav.install" => "Install",
            "nav.terminal" => "Open terminal",
            "confirm.heading" => "Confirm installation",
            "confirm.body" => "All data on the selected disk will be permanently erased and the system will be installed. This cannot be undone.",
            "confirm.body.manual" => "The staged partition changes will now be written to disk and Sirius will be installed. Formatted or deleted data cannot be recovered.",
            "confirm.cancel" => "Cancel",
            "confirm.install" => "Erase disk and install",
            "welcome.title" => "Welcome",
            "welcome.desc" => "This assistant will guide you through installation.",
            "network.title" => "Network",
            "network.desc" => "Connect to a network. A connection is optional but recommended.",
            "network.body" => "Choose a Wi-Fi network below.",
            "network.available" => "Available Wi-Fi networks",
            "network.scanning" => "Looking for networks…",
            "network.connect" => "Connect",
            "network.connected" => "Connected",
            "network.refresh" => "Scan again",
            "network.open" => "Open network",
            "network.wpa" => "WPA/WPA2 Personal",
            "network.wpa3" => "WPA3 Personal",
            "network.unsupported" => "Enterprise or legacy security is not supported here",
            "network.connect_to" => "Connect to",
            "network.password" => "Enter the Wi-Fi password.",
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
            "summary.desc" => "Review your choices before writing changes to disk.",
            "summary.language" => "Language",
            "summary.keyboard" => "Keyboard",
            "summary.timezone" => "Time zone",
            "summary.disk" => "Disk",
            "summary.encryption" => "Encryption",
            "summary.user" => "User",
            "summary.hostname" => "Hostname",
            "summary.enabled" => "enabled",
            "summary.disabled" => "disabled",
            "summary.manual" => "manual layout",
            "storage.title" => "Storage",
            "storage.desc" => "Choose a disk and how Sirius should use it. Changes are applied only after confirmation.",
            "storage.disk" => "Destination disk",
            "storage.automatic_mode" => "Automatic partitioning",
            "storage.automatic_mode.desc" => "Erase the disk and create the needed partitions automatically.",
            "storage.erase_notice" => "All data on this disk will be erased.",
            "storage.discard" => "Discard changes",
            "storage.no_disk_selected" => "Select a disk to continue.",
            "storage.layout" => "Disk layout",
            "storage.table" => "Table",
            "storage.close" => "Close",
            "storage.done" => "Done",
            "storage.delete" => "Delete Partition",
            "storage.not_mounted" => "Not mounted",
            "storage.unformatted" => "Unformatted",
            "storage.unknown" => "Unknown",
            "storage.new_partition" => "New Sirius Partition",
            "storage.efi_partition" => "EFI System Partition",
            "storage.root_partition" => "Sirius Root",
            "storage.swap_partition" => "Swap",
            "storage.volumes" => "Volumes and partitions",
            "storage.free" => "Unallocated space",
            "storage.pending" => "New partition pending",
            "storage.edit" => "Edit partition",
            "storage.create" => "Create partition",
            "storage.apply" => "Apply",
            "storage.filesystem" => "Filesystem",
            "storage.size" => "Size (GiB)",
            "storage.mount.placeholder" => "Mount point: / or /boot/efi",
            "storage.mount" => "Mount Point",
            "storage.label" => "Label",
            "storage.save_changes" => "Save Changes",
            "storage.unavailable" => "Disks could not be loaded",
            "storage.none" => "No available disks",
            "storage.none.desc" => "Connect a disk or unmount its filesystems and reopen Sirius.",
            "storage.in_use" => "in use",
            "diagnostics.title" => "System compatibility",
            "diagnostics.desc" => "Sirius checked your hardware before installing.",
            "disk.title" => "Select a disk",
            "disk.desc" => "The chosen disk will be erased.",
            "disk.none" => "No disks found",
            "progress.title" => "Installing the system",
            "progress.logs" => "Show install log",
            "progress.failed.title" => "Installation failed",
            "progress.failed.desc" => "Something went wrong during installation. Check the log below for details. No changes were finished — you can reboot and try again.",
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
            "nav.install" => "Instalar",
            "nav.terminal" => "Abrir terminal",
            "confirm.heading" => "Confirmar instalação",
            "confirm.body" => "Todos os dados do disco selecionado serão apagados permanentemente e o sistema será instalado. Isso não pode ser desfeito.",
            "confirm.body.manual" => "As mudanças de partição agendadas serão gravadas no disco e o Sirius será instalado. Dados formatados ou excluídos não poderão ser recuperados.",
            "confirm.cancel" => "Cancelar",
            "confirm.install" => "Apagar disco e instalar",
            "welcome.title" => "Bem-vindo",
            "welcome.desc" => "Este assistente vai guiá-lo pela instalação.",
            "network.title" => "Rede",
            "network.desc" => "Conecte-se a uma rede. A conexão é opcional, mas recomendada.",
            "network.body" => "Escolha uma rede Wi-Fi abaixo.",
            "network.available" => "Redes Wi-Fi disponíveis",
            "network.scanning" => "Procurando redes…",
            "network.connect" => "Conectar",
            "network.connected" => "Conectado",
            "network.refresh" => "Procurar novamente",
            "network.open" => "Rede aberta",
            "network.wpa" => "WPA/WPA2 pessoal",
            "network.wpa3" => "WPA3 pessoal",
            "network.unsupported" => "Segurança enterprise ou legada não é suportada aqui",
            "network.connect_to" => "Conectar a",
            "network.password" => "Digite a senha do Wi-Fi.",
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
            "summary.desc" => "Revise suas escolhas antes de gravar as mudanças no disco.",
            "summary.language" => "Idioma",
            "summary.keyboard" => "Teclado",
            "summary.timezone" => "Fuso horário",
            "summary.disk" => "Disco",
            "summary.encryption" => "Criptografia",
            "summary.user" => "Usuário",
            "summary.hostname" => "Nome da máquina",
            "summary.enabled" => "ativada",
            "summary.disabled" => "desativada",
            "summary.manual" => "layout manual",
            "storage.title" => "Armazenamento",
            "storage.desc" => "Escolha um disco e como o Sirius deve usá-lo. As mudanças só serão aplicadas após a confirmação.",
            "storage.disk" => "Disco de destino",
            "storage.automatic_mode" => "Particionamento automático",
            "storage.automatic_mode.desc" => "Apagar o disco e criar as partições necessárias automaticamente.",
            "storage.erase_notice" => "Todos os dados deste disco serão apagados.",
            "storage.discard" => "Descartar alterações",
            "storage.no_disk_selected" => "Selecione um disco para continuar.",
            "storage.layout" => "Mapa do disco",
            "storage.table" => "Tabela",
            "storage.close" => "Fechar",
            "storage.done" => "Concluído",
            "storage.delete" => "Excluir Partição",
            "storage.not_mounted" => "Não montado",
            "storage.unformatted" => "Não formatado",
            "storage.unknown" => "Desconhecido",
            "storage.new_partition" => "Nova Partição Sirius",
            "storage.efi_partition" => "Sistema EFI",
            "storage.root_partition" => "Raiz do Sirius",
            "storage.swap_partition" => "Memória Virtual",
            "storage.volumes" => "Volumes e partições",
            "storage.free" => "Espaço não alocado",
            "storage.pending" => "Nova partição pendente",
            "storage.edit" => "Editar partição",
            "storage.create" => "Criar partição",
            "storage.apply" => "Aplicar",
            "storage.filesystem" => "Sistema de arquivos",
            "storage.size" => "Tamanho (GiB)",
            "storage.mount.placeholder" => "Ponto de montagem: / ou /boot/efi",
            "storage.mount" => "Ponto de Montagem",
            "storage.label" => "Rótulo",
            "storage.save_changes" => "Salvar Alterações",
            "storage.unavailable" => "Não foi possível carregar os discos",
            "storage.none" => "Nenhum disco disponível",
            "storage.none.desc" => "Conecte um disco ou desmonte seus sistemas de arquivos e reabra o Sirius.",
            "storage.in_use" => "em uso",
            "diagnostics.title" => "Compatibilidade do sistema",
            "diagnostics.desc" => "O Sirius verificou seu hardware antes de instalar.",
            "disk.title" => "Selecione um disco",
            "disk.desc" => "O disco escolhido será apagado.",
            "disk.none" => "Nenhum disco encontrado",
            "progress.title" => "Instalando o sistema",
            "progress.logs" => "Mostrar registro da instalação",
            "progress.failed.title" => "Falha na instalação",
            "progress.failed.desc" => "Algo deu errado durante a instalação. Veja o registro abaixo para detalhes. Nada foi concluído — você pode reiniciar e tentar de novo.",
            "finished.title" => "Instalação concluída",
            "finished.desc" => "O sistema foi instalado. Reinicie para começar a usá-lo.",
            "finished.restart" => "Reiniciar agora",
            _ => "",
        }
    };
    match lang {
        Lang::PtBr => {
            let p = pt(key);
            if !p.is_empty() {
                p
            } else {
                let e = en(key);
                if e.is_empty() {
                    leak_key(key)
                } else {
                    e
                }
            }
        }
        Lang::En => {
            let e = en(key);
            if e.is_empty() {
                leak_key(key)
            } else {
                e
            }
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
