//! User API keys against a real PostgreSQL with row-level security.

mod common;

use std::sync::Arc;

use common::Env;
use factory_accounts::{AccountId, AesGcmSecretStore};
use factory_broker::*;

const PLAIN: &str = "sk-ant-api03-SUPERSECRETVALUE-wxyz";

fn store() -> KeyStore {
    KeyStore::new(Arc::new(AesGcmSecretStore::new(&[9u8; 32])))
}

fn anthropic(label: &str) -> NewKey {
    NewKey {
        provider: KeyProvider::Anthropic,
        label: label.into(),
        base_url: None,
        plaintext: SecretString::new(PLAIN),
    }
}

async fn save(
    env: &Env,
    ks: &KeyStore,
    who: AccountId,
    new: NewKey,
) -> Result<KeyRecord, KeyError> {
    let mut tx = env.tx(who).await;
    let r = ks.save(&mut tx, who, new).await?;
    tx.commit().await.unwrap();
    Ok(r)
}

#[tokio::test]
async fn keys_are_sealed_at_rest_and_open_for_the_owner() {
    let env = Env::new().await;
    let ks = store();
    let alice = env.account("alice").await;
    let rec = save(&env, &ks, alice, anthropic("main key")).await.unwrap();
    assert_eq!(rec.key_hint, "wxyz");
    assert_eq!(rec.provider, KeyProvider::Anthropic);
    assert!(rec.last_used_at.is_none());

    // The stored bytes do not contain the plaintext (nor its tail).
    let sealed: Vec<u8> = sqlx::query_scalar("SELECT sealed FROM user_api_keys WHERE id = $1")
        .bind(rec.id.0)
        .fetch_one(&env.admin)
        .await
        .unwrap();
    assert!(!sealed.windows(8).any(|w| w == &PLAIN.as_bytes()[..8]));
    assert!(!sealed.windows(10).any(|w| w == b"SUPERSECRE"));
    let text_columns: String =
        sqlx::query_scalar("SELECT (user_api_keys.*)::text FROM user_api_keys WHERE id = $1")
            .bind(rec.id.0)
            .fetch_one(&env.admin)
            .await
            .unwrap();
    assert!(!text_columns.contains("SUPERSECRET"));

    let mut tx = env.tx(alice).await;
    let opened = ks.open_for_owner(&mut tx, alice, rec.id).await.unwrap();
    assert_eq!(opened.expose(), PLAIN);
    let listed = ks.list(&mut tx, alice).await.unwrap();
    assert_eq!(listed.len(), 1);
    assert!(listed[0].last_used_at.is_some());
}

#[tokio::test]
async fn another_account_cannot_list_open_revoke_or_delete() {
    let env = Env::new().await;
    let ks = store();
    let alice = env.account("alice").await;
    let bob = env.account("bob").await;
    let rec = save(&env, &ks, alice, anthropic("alice's")).await.unwrap();

    let mut tx = env.tx(bob).await;
    assert!(ks.list(&mut tx, bob).await.unwrap().is_empty());
    // Even naming alice as the account: RLS hides her rows.
    assert!(ks.list(&mut tx, alice).await.unwrap().is_empty());
    assert!(matches!(
        ks.open_for_owner(&mut tx, bob, rec.id).await,
        Err(KeyError::NotFound)
    ));
    assert!(matches!(
        ks.open_for_owner(&mut tx, alice, rec.id).await,
        Err(KeyError::NotFound)
    ));
    assert!(matches!(
        ks.revoke(&mut tx, bob, rec.id).await,
        Err(KeyError::NotFound)
    ));
    assert!(matches!(
        ks.delete(&mut tx, bob, rec.id).await,
        Err(KeyError::NotFound)
    ));
    assert!(matches!(
        ks.delete(&mut tx, alice, rec.id).await,
        Err(KeyError::NotFound)
    ));
    drop(tx);
    // Bob cannot insert a key for Alice either.
    let mut tx = env.tx(bob).await;
    assert!(ks.save(&mut tx, alice, anthropic("forged")).await.is_err());
    drop(tx);
    assert_eq!(env.count("SELECT count(*) FROM user_api_keys").await, 1);
}

#[tokio::test]
async fn a_sealed_value_moved_to_another_row_or_account_does_not_open() {
    let env = Env::new().await;
    let ks = store();
    let alice = env.account("alice").await;
    let bob = env.account("bob").await;
    let a1 = save(&env, &ks, alice, anthropic("a1")).await.unwrap();
    let mut other = anthropic("a2");
    other.plaintext = SecretString::new("sk-ant-other-key-0000");
    let a2 = save(&env, &ks, alice, other).await.unwrap();
    let b1 = save(&env, &ks, bob, anthropic("b1")).await.unwrap();

    // Same account, different row.
    sqlx::query("UPDATE user_api_keys SET sealed = (SELECT sealed FROM user_api_keys WHERE id = $1) WHERE id = $2")
        .bind(a1.id.0)
        .bind(a2.id.0)
        .execute(&env.admin)
        .await
        .unwrap();
    let mut tx = env.tx(alice).await;
    assert!(matches!(
        ks.open_for_owner(&mut tx, alice, a2.id).await,
        Err(KeyError::Decrypt)
    ));
    assert_eq!(
        ks.open_for_owner(&mut tx, alice, a1.id)
            .await
            .unwrap()
            .expose(),
        PLAIN
    );
    drop(tx);

    // Another account.
    sqlx::query("UPDATE user_api_keys SET sealed = (SELECT sealed FROM user_api_keys WHERE id = $1) WHERE id = $2")
        .bind(a1.id.0)
        .bind(b1.id.0)
        .execute(&env.admin)
        .await
        .unwrap();
    let mut tx = env.tx(bob).await;
    assert!(matches!(
        ks.open_for_owner(&mut tx, bob, b1.id).await,
        Err(KeyError::Decrypt)
    ));
    drop(tx);

    // A different provider on the same row also fails (the provider is in the context).
    sqlx::query("UPDATE user_api_keys SET provider = 'openai' WHERE id = $1")
        .bind(a1.id.0)
        .execute(&env.admin)
        .await
        .unwrap();
    let mut tx = env.tx(alice).await;
    assert!(matches!(
        ks.open_for_owner(&mut tx, alice, a1.id).await,
        Err(KeyError::Decrypt)
    ));
}

#[tokio::test]
async fn revoke_blocks_opening_and_delete_removes_the_row() {
    let env = Env::new().await;
    let ks = store();
    let alice = env.account("alice").await;
    let rec = save(&env, &ks, alice, anthropic("k")).await.unwrap();

    let mut tx = env.tx(alice).await;
    let revoked = ks.revoke(&mut tx, alice, rec.id).await.unwrap();
    assert!(revoked.revoked_at.is_some());
    assert!(matches!(
        ks.open_for_owner(&mut tx, alice, rec.id).await,
        Err(KeyError::Revoked)
    ));
    assert_eq!(ks.list(&mut tx, alice).await.unwrap().len(), 1);
    ks.delete(&mut tx, alice, rec.id).await.unwrap();
    assert!(ks.list(&mut tx, alice).await.unwrap().is_empty());
    tx.commit().await.unwrap();
    assert_eq!(env.count("SELECT count(*) FROM user_api_keys").await, 0);
}

#[tokio::test]
async fn validation_and_self_hosted_base_url() {
    let env = Env::new().await;
    let ks = store();
    let alice = env.account("alice").await;
    for (label, base, key) in [
        ("", None, PLAIN),
        ("ok", Some("https://x.example"), PLAIN),
        ("ok", None, "short"),
        ("ok", None, "has space in it 12345"),
    ] {
        let mut k = anthropic(label);
        k.base_url = base.map(str::to_owned);
        k.plaintext = SecretString::new(key);
        let err = save(&env, &ks, alice, k).await.unwrap_err();
        assert!(
            matches!(err, KeyError::Invalid(_)),
            "{label:?} {base:?}: {err:?}"
        );
        assert!(!err.to_string().contains("short"));
    }
    for bad in [
        None,
        Some("ftp://x"),
        Some("https://user:pw@host/v1"),
        Some("https://h/v1?k=1"),
        Some("https://"),
    ] {
        let k = NewKey {
            provider: KeyProvider::OpenaiCompatible,
            label: "local".into(),
            base_url: bad.map(str::to_owned),
            plaintext: SecretString::new("local-key-12345"),
        };
        assert!(
            matches!(save(&env, &ks, alice, k).await, Err(KeyError::Invalid(_))),
            "{bad:?}"
        );
    }
    let k = NewKey {
        provider: KeyProvider::OpenaiCompatible,
        label: "local".into(),
        base_url: Some("http://ollama.internal:11434/v1".into()),
        plaintext: SecretString::new("local-key-12345"),
    };
    let rec = save(&env, &ks, alice, k).await.unwrap();
    assert_eq!(
        rec.base_url.as_deref(),
        Some("http://ollama.internal:11434/v1")
    );
    assert_eq!(rec.key_hint, "2345");
}

#[tokio::test]
async fn no_secret_text_in_debug_output_or_errors() {
    let env = Env::new().await;
    let ks = store();
    let alice = env.account("alice").await;
    let new = anthropic("dbg");
    let dbg_new = format!("{new:?}");
    assert!(
        !dbg_new.contains("SUPERSECRET") && dbg_new.contains("[REDACTED]"),
        "{dbg_new}"
    );
    assert_eq!(format!("{:?}", SecretString::new(PLAIN)), "[REDACTED]");

    let rec = save(&env, &ks, alice, new).await.unwrap();
    let dbg_rec = format!("{rec:?}");
    assert!(!dbg_rec.contains("SUPERSECRET") && !dbg_rec.contains(PLAIN));
    assert!(!serde_json::to_string(&rec).unwrap().contains("SUPERSECRET"));

    let mut tx = env.tx(alice).await;
    let opened = ks.open_for_owner(&mut tx, alice, rec.id).await.unwrap();
    assert!(!format!("{opened:?}").contains("SUPERSECRET"));
    ks.revoke(&mut tx, alice, rec.id).await.unwrap();
    let err = ks.open_for_owner(&mut tx, alice, rec.id).await.unwrap_err();
    assert!(!format!("{err:?} {err}").contains("SUPERSECRET"));
}

#[tokio::test]
async fn a_different_master_key_cannot_open() {
    let env = Env::new().await;
    let alice = env.account("alice").await;
    let rec = save(&env, &store(), alice, anthropic("k")).await.unwrap();
    let other = KeyStore::new(Arc::new(AesGcmSecretStore::new(&[1u8; 32])));
    let mut tx = env.tx(alice).await;
    assert!(matches!(
        other.open_for_owner(&mut tx, alice, rec.id).await,
        Err(KeyError::Decrypt)
    ));
}
