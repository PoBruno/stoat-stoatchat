use crate::{
    calculate_channel_permissions, calculate_user_permissions, ChannelPermission, ChannelType,
    Override, PermissionQuery, RelationshipStatus, DEFAULT_PERMISSION_DIRECT_MESSAGE,
    DEFAULT_PERMISSION_SERVER, DEFAULT_PERMISSION_VIEW_ONLY,
};

#[tokio::test]
async fn validate_user_permissions() {
    /// Scenario in which we are friends with a user
    /// and we have a DM channel open with them
    struct Scenario {}
    let mut query = Scenario {};

    let perms = calculate_user_permissions(&mut query).await;
    assert!(perms.has(u64::MAX));

    let perms = calculate_channel_permissions(&mut query).await;
    let value: u64 = perms.into();
    assert_eq!(value, *DEFAULT_PERMISSION_DIRECT_MESSAGE);

    #[async_trait]
    impl PermissionQuery for Scenario {
        async fn are_we_privileged(&mut self) -> bool {
            false
        }

        async fn are_we_a_bot(&mut self) -> bool {
            false
        }

        async fn are_the_users_same(&mut self) -> bool {
            false
        }

        async fn user_relationship(&mut self) -> RelationshipStatus {
            RelationshipStatus::Friend
        }

        async fn user_is_bot(&mut self) -> bool {
            false
        }

        async fn have_mutual_connection(&mut self) -> bool {
            false
        }

        async fn are_we_server_owner(&mut self) -> bool {
            unreachable!()
        }

        async fn are_we_a_member(&mut self) -> bool {
            unreachable!()
        }

        async fn get_default_server_permissions(&mut self) -> u64 {
            unreachable!()
        }

        async fn get_our_server_role_overrides(&mut self) -> Vec<Override> {
            unreachable!()
        }

        async fn are_we_timed_out(&mut self) -> bool {
            unreachable!()
        }

        async fn do_we_have_publish_overwrites(&mut self) -> bool {
            true
        }

        async fn do_we_have_receive_overwrites(&mut self) -> bool {
            true
        }

        async fn get_channel_type(&mut self) -> ChannelType {
            ChannelType::DirectMessage
        }

        async fn get_default_channel_permissions(&mut self) -> Override {
            unreachable!()
        }

        async fn get_our_channel_role_overrides(&mut self) -> Vec<Override> {
            unreachable!()
        }

        async fn do_we_own_the_channel(&mut self) -> bool {
            unreachable!()
        }

        async fn are_we_part_of_the_channel(&mut self) -> bool {
            true
        }

        async fn set_recipient_as_user(&mut self) {
            // no-op
        }

        async fn set_server_from_channel(&mut self) {
            unreachable!()
        }
    }
}

#[tokio::test]
async fn validate_group_permissions() {
    /// Scenario in which we are in a group channel with only talking permission
    struct Scenario {}
    let mut query = Scenario {};

    let perms = calculate_channel_permissions(&mut query).await;
    let value: u64 = perms.into();
    assert_eq!(
        value,
        *DEFAULT_PERMISSION_VIEW_ONLY | ChannelPermission::SendMessage as u64
    );

    #[async_trait]
    impl PermissionQuery for Scenario {
        async fn are_we_privileged(&mut self) -> bool {
            false
        }

        async fn are_we_a_bot(&mut self) -> bool {
            unreachable!()
        }

        async fn are_the_users_same(&mut self) -> bool {
            unreachable!()
        }

        async fn user_relationship(&mut self) -> RelationshipStatus {
            unreachable!()
        }

        async fn user_is_bot(&mut self) -> bool {
            unreachable!()
        }

        async fn have_mutual_connection(&mut self) -> bool {
            unreachable!()
        }

        async fn are_we_server_owner(&mut self) -> bool {
            unreachable!()
        }

        async fn are_we_a_member(&mut self) -> bool {
            unreachable!()
        }

        async fn get_default_server_permissions(&mut self) -> u64 {
            unreachable!()
        }

        async fn get_our_server_role_overrides(&mut self) -> Vec<Override> {
            unreachable!()
        }

        async fn are_we_timed_out(&mut self) -> bool {
            unreachable!()
        }

        async fn do_we_have_publish_overwrites(&mut self) -> bool {
            true
        }

        async fn do_we_have_receive_overwrites(&mut self) -> bool {
            true
        }

        async fn get_channel_type(&mut self) -> ChannelType {
            ChannelType::Group
        }

        async fn get_default_channel_permissions(&mut self) -> Override {
            Override {
                allow: ChannelPermission::SendMessage as u64,
                deny: 0,
            }
        }

        async fn get_our_channel_role_overrides(&mut self) -> Vec<Override> {
            unreachable!()
        }

        async fn do_we_own_the_channel(&mut self) -> bool {
            false
        }

        async fn are_we_part_of_the_channel(&mut self) -> bool {
            true
        }

        async fn set_recipient_as_user(&mut self) {
            unreachable!()
        }

        async fn set_server_from_channel(&mut self) {
            unreachable!()
        }
    }
}

#[tokio::test]
async fn validate_server_permissions() {
    /// Scenario in which we are in a server channel where:
    /// - the server grants reading history and sending messages by default
    /// - we have a role that allows us to upload files and react but denies reading history
    /// - however the channel disallows sending messages
    /// - and removes our role specific react permission
    struct Scenario {}
    let mut query = Scenario {};

    let perms = calculate_channel_permissions(&mut query).await;
    let value: u64 = perms.into();
    assert_eq!(
        value,
        ChannelPermission::ViewChannel as u64 | ChannelPermission::UploadFiles as u64
    );

    #[async_trait]
    impl PermissionQuery for Scenario {
        async fn are_we_privileged(&mut self) -> bool {
            false
        }

        async fn are_we_a_bot(&mut self) -> bool {
            unreachable!()
        }

        async fn are_the_users_same(&mut self) -> bool {
            unreachable!()
        }

        async fn user_relationship(&mut self) -> RelationshipStatus {
            unreachable!()
        }

        async fn user_is_bot(&mut self) -> bool {
            unreachable!()
        }

        async fn have_mutual_connection(&mut self) -> bool {
            unreachable!()
        }

        async fn are_we_server_owner(&mut self) -> bool {
            false
        }

        async fn are_we_a_member(&mut self) -> bool {
            true
        }

        async fn get_default_server_permissions(&mut self) -> u64 {
            ChannelPermission::ViewChannel as u64
                | ChannelPermission::SendMessage as u64
                | ChannelPermission::ReadMessageHistory as u64
        }

        async fn get_our_server_role_overrides(&mut self) -> Vec<Override> {
            vec![Override {
                allow: ChannelPermission::UploadFiles as u64 | ChannelPermission::React as u64,
                deny: ChannelPermission::ReadMessageHistory as u64,
            }]
        }

        async fn are_we_timed_out(&mut self) -> bool {
            false
        }

        async fn do_we_have_publish_overwrites(&mut self) -> bool {
            true
        }

        async fn do_we_have_receive_overwrites(&mut self) -> bool {
            true
        }

        async fn get_channel_type(&mut self) -> ChannelType {
            ChannelType::ServerChannel
        }

        async fn get_default_channel_permissions(&mut self) -> Override {
            Override {
                allow: 0,
                deny: ChannelPermission::SendMessage as u64,
            }
        }

        async fn get_our_channel_role_overrides(&mut self) -> Vec<Override> {
            vec![Override {
                allow: 0,
                deny: ChannelPermission::React as u64,
            }]
        }

        async fn do_we_own_the_channel(&mut self) -> bool {
            unreachable!()
        }

        async fn are_we_part_of_the_channel(&mut self) -> bool {
            unreachable!()
        }

        async fn set_recipient_as_user(&mut self) {
            unreachable!()
        }

        async fn set_server_from_channel(&mut self) {
            // no-op
        }
    }
}

#[tokio::test]
async fn validate_timed_out_member() {
    /// Scenario in which we are in a server that we have been timed out from
    struct Scenario {}
    let mut query = Scenario {};

    let perms = calculate_channel_permissions(&mut query).await;
    let value: u64 = perms.into();
    assert_eq!(value, *DEFAULT_PERMISSION_VIEW_ONLY);

    #[async_trait]
    impl PermissionQuery for Scenario {
        async fn are_we_privileged(&mut self) -> bool {
            false
        }

        async fn are_we_a_bot(&mut self) -> bool {
            unreachable!()
        }

        async fn are_the_users_same(&mut self) -> bool {
            unreachable!()
        }

        async fn user_relationship(&mut self) -> RelationshipStatus {
            unreachable!()
        }

        async fn user_is_bot(&mut self) -> bool {
            unreachable!()
        }

        async fn have_mutual_connection(&mut self) -> bool {
            unreachable!()
        }

        async fn are_we_server_owner(&mut self) -> bool {
            false
        }

        async fn are_we_a_member(&mut self) -> bool {
            true
        }

        async fn get_default_server_permissions(&mut self) -> u64 {
            *DEFAULT_PERMISSION_SERVER
        }

        async fn get_our_server_role_overrides(&mut self) -> Vec<Override> {
            vec![]
        }

        async fn are_we_timed_out(&mut self) -> bool {
            true
        }

        async fn do_we_have_publish_overwrites(&mut self) -> bool {
            true
        }

        async fn do_we_have_receive_overwrites(&mut self) -> bool {
            true
        }

        async fn get_channel_type(&mut self) -> ChannelType {
            ChannelType::ServerChannel
        }

        async fn get_default_channel_permissions(&mut self) -> Override {
            Override { allow: 0, deny: 0 }
        }

        async fn get_our_channel_role_overrides(&mut self) -> Vec<Override> {
            vec![]
        }

        async fn do_we_own_the_channel(&mut self) -> bool {
            unreachable!()
        }

        async fn are_we_part_of_the_channel(&mut self) -> bool {
            unreachable!()
        }

        async fn set_recipient_as_user(&mut self) {
            unreachable!()
        }

        async fn set_server_from_channel(&mut self) {
            // no-op
        }
    }
}

#[tokio::test]
async fn validate_channel_default_below_server_roles() {
    /// Scenario where:
    /// - Server default allows ViewChannel.
    /// - Channel default override denies SendMessage.
    /// - Server role override allows SendMessage.
    /// - We don't have channel role overrides.
    /// Since default permissions should be below role permissions,
    /// our server role override (allowing SendMessage) should take precedence
    /// over the channel default permission (denying SendMessage).
    struct Scenario {}
    let mut query = Scenario {};

    let perms = calculate_channel_permissions(&mut query).await;
    let value: u64 = perms.into();
    assert_eq!(
        value,
        ChannelPermission::ViewChannel as u64 | ChannelPermission::SendMessage as u64
    );

    #[async_trait]
    impl PermissionQuery for Scenario {
        async fn are_we_privileged(&mut self) -> bool {
            false
        }

        async fn are_we_a_bot(&mut self) -> bool {
            unreachable!()
        }

        async fn are_the_users_same(&mut self) -> bool {
            unreachable!()
        }

        async fn user_relationship(&mut self) -> RelationshipStatus {
            unreachable!()
        }

        async fn user_is_bot(&mut self) -> bool {
            unreachable!()
        }

        async fn have_mutual_connection(&mut self) -> bool {
            unreachable!()
        }

        async fn are_we_server_owner(&mut self) -> bool {
            false
        }

        async fn are_we_a_member(&mut self) -> bool {
            true
        }

        async fn get_default_server_permissions(&mut self) -> u64 {
            ChannelPermission::ViewChannel as u64
        }

        async fn get_our_server_role_overrides(&mut self) -> Vec<Override> {
            vec![Override {
                allow: ChannelPermission::SendMessage as u64,
                deny: 0,
            }]
        }

        async fn are_we_timed_out(&mut self) -> bool {
            false
        }

        async fn do_we_have_publish_overwrites(&mut self) -> bool {
            true
        }

        async fn do_we_have_receive_overwrites(&mut self) -> bool {
            true
        }

        async fn get_channel_type(&mut self) -> ChannelType {
            ChannelType::ServerChannel
        }

        async fn get_default_channel_permissions(&mut self) -> Override {
            Override {
                allow: 0,
                deny: ChannelPermission::SendMessage as u64,
            }
        }

        async fn get_our_channel_role_overrides(&mut self) -> Vec<Override> {
            vec![]
        }

        async fn do_we_own_the_channel(&mut self) -> bool {
            unreachable!()
        }

        async fn are_we_part_of_the_channel(&mut self) -> bool {
            unreachable!()
        }

        async fn set_recipient_as_user(&mut self) {
            unreachable!()
        }

        async fn set_server_from_channel(&mut self) {
            // no-op
        }
    }
}

/// Os bits de soundboard e musicbox precisam sair concedidos por padrao.
///
/// Nao e detalhe: `UseSoundboard` foi posto no padrao sem migration que o
/// propagasse, e ficou negado em toda instancia que ja existia. O sintoma era
/// "o botao nao funciona" sem erro nenhum. Este teste ancora a intencao.
#[tokio::test]
async fn soundboard_e_musicbox_vem_no_padrao() {
    assert!(
        (*DEFAULT_PERMISSION_SERVER) & (ChannelPermission::UseSoundboard as u64) != 0,
        "usar o soundboard deveria vir concedido"
    );
    assert!(
        (*DEFAULT_PERMISSION_SERVER) & (ChannelPermission::UseMusicBox as u64) != 0,
        "usar o musicbox deveria vir concedido"
    );
    assert!(
        (*DEFAULT_PERMISSION_SERVER) & (ChannelPermission::ManageSoundboard as u64) == 0,
        "gerir sons NAO deve vir concedido: e o dono quem decide quem sobe som"
    );
}

/// Os bits novos nao podem colidir com nada existente nem passar de 52.
///
/// O comentario do enum limita a 52 bits por causa do JavaScript, onde
/// inteiro seguro para em 2^53. Um bit acima disso viajaria errado ate o
/// navegador sem erro nenhum.
#[tokio::test]
async fn bits_novos_sao_unicos_e_seguros() {
    let bits = [
        ("UseSoundboard", ChannelPermission::UseSoundboard as u64),
        ("ManageSoundboard", ChannelPermission::ManageSoundboard as u64),
        ("UseMusicBox", ChannelPermission::UseMusicBox as u64),
    ];

    for (nome, bit) in bits {
        assert_eq!(bit.count_ones(), 1, "{nome} deveria ser um bit so");
        assert!(
            bit <= (1u64 << 52),
            "{nome} passa dos 52 bits e o JavaScript o corromperia"
        );
        assert!(
            (ChannelPermission::GrantAllSafe as u64) & bit != 0,
            "{nome} precisa caber em GrantAllSafe, senao nem o dono o teria"
        );
    }

    // Distintos entre si.
    assert_ne!(bits[0].1, bits[1].1);
    assert_ne!(bits[1].1, bits[2].1);
    assert_ne!(bits[0].1, bits[2].1);
}

/// Negar o bit do musicbox realmente tira a permissao.
#[tokio::test]
async fn negar_musicbox_tira_so_o_musicbox() {
    struct Scenario {}
    let mut query = Scenario {};

    let perms = calculate_channel_permissions(&mut query).await;

    assert!(
        !perms.has_channel_permission(ChannelPermission::UseMusicBox),
        "o override negou UseMusicBox; ele nao pode sobrar"
    );
    assert!(
        perms.has_channel_permission(ChannelPermission::Connect),
        "negar musicbox nao pode derrubar a entrada na chamada"
    );
    assert!(
        perms.has_channel_permission(ChannelPermission::UseSoundboard),
        "negar musicbox nao pode derrubar o soundboard"
    );

    #[async_trait]
    impl PermissionQuery for Scenario {
        async fn are_we_privileged(&mut self) -> bool {
            false
        }

        async fn are_we_a_bot(&mut self) -> bool {
            false
        }

        async fn are_the_users_same(&mut self) -> bool {
            unreachable!()
        }

        async fn user_relationship(&mut self) -> RelationshipStatus {
            unreachable!()
        }

        async fn user_is_bot(&mut self) -> bool {
            unreachable!()
        }

        async fn have_mutual_connection(&mut self) -> bool {
            unreachable!()
        }

        async fn are_we_server_owner(&mut self) -> bool {
            false
        }

        async fn are_we_a_member(&mut self) -> bool {
            true
        }

        async fn get_default_server_permissions(&mut self) -> u64 {
            *DEFAULT_PERMISSION_SERVER
        }

        async fn get_our_server_role_overrides(&mut self) -> Vec<Override> {
            vec![]
        }

        async fn are_we_timed_out(&mut self) -> bool {
            false
        }

        async fn do_we_have_publish_overwrites(&mut self) -> bool {
            true
        }

        async fn do_we_have_receive_overwrites(&mut self) -> bool {
            true
        }

        async fn get_channel_type(&mut self) -> ChannelType {
            ChannelType::ServerChannel
        }

        async fn get_default_channel_permissions(&mut self) -> Override {
            Override {
                allow: 0,
                deny: ChannelPermission::UseMusicBox as u64,
            }
        }

        async fn get_our_channel_role_overrides(&mut self) -> Vec<Override> {
            vec![]
        }

        async fn do_we_own_the_channel(&mut self) -> bool {
            false
        }

        async fn are_we_part_of_the_channel(&mut self) -> bool {
            true
        }

        async fn set_recipient_as_user(&mut self) {
            unreachable!()
        }

        async fn set_server_from_channel(&mut self) {}
    }
}
