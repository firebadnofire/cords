use super::*;

#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn directory_accounts_paginate_group_devices_and_recheck_sessions() -> Result<()> {
    let (service, _directory) = fixture().await?;
    let (owner, owner_contact) = identity()?;
    let session = join(&service, &owner, owner_contact.clone()).await?;
    // A second independently signed device belongs to the same account.
    let (second, mut contact) = identity()?;
    let mut authorization = second.authorization.value.clone();
    authorization.generation = 2;
    authorization.previous_record_hash = Some(hash(canonical(&owner.authorization)?));
    authorization
        .account_id
        .clone_from(&owner.authorization.value.account_id);
    authorization
        .root_public_key
        .clone_from(&owner.authorization.value.root_public_key);
    contact.authorization = owner.sign_root(authorization)?;
    let mut binding = contact.mls_binding.value;
    binding
        .account_id
        .clone_from(&owner.authorization.value.account_id);
    binding.authorization_hash = hash(canonical(&contact.authorization)?);
    contact.mls_binding = second.sign_device(binding)?;
    validate_contact(&contact, now()?)?;
    join(&service, &second, contact).await?;
    for _ in 0..17 {
        let (identity, contact) = identity()?;
        join(&service, &identity, contact).await?;
    }
    let page = super::super::directory::accounts(
        State(service.clone()),
        headers(&session)?,
        Query(MembershipPage::default()),
    )
    .await?
    .0;
    assert_eq!(page.accounts.len(), 16);
    let rest = super::super::directory::accounts(
        State(service.clone()),
        headers(&session)?,
        Query(MembershipPage {
            after: page.next_after.clone().context("missing continuation")?,
            ..MembershipPage::default()
        }),
    )
    .await?
    .0;
    assert_eq!(rest.accounts.len(), 2);
    assert!(rest.next_after.is_none());
    let all: Vec<_> = page.accounts.iter().chain(&rest.accounts).collect();
    assert!(
        all.windows(2)
            .all(|pair| pair[0].account_id < pair[1].account_id)
    );
    assert_eq!(
        all.iter()
            .find(|account| account.account_id == owner.authorization.value.account_id)
            .context("missing grouped owner")?
            .devices
            .len(),
        2
    );
    assert!(
        super::super::directory::accounts(
            State(service.clone()),
            HeaderMap::new(),
            Query(MembershipPage::default())
        )
        .await
        .is_err()
    );
    sqlx::query("UPDATE memberships SET active=false WHERE device_id<>$1")
        .bind(&session.membership.value.device_id)
        .execute(service.0.store.pool())
        .await?;
    let remaining = super::super::directory::accounts(
        State(service.clone()),
        headers(&session)?,
        Query(MembershipPage::default()),
    )
    .await?
    .0;
    assert_eq!(remaining.accounts.len(), 1);
    assert_eq!(remaining.accounts[0].devices.len(), 1);
    sqlx::query("UPDATE sessions SET active=false")
        .execute(service.0.store.pool())
        .await?;
    assert!(
        super::super::directory::accounts(
            State(service),
            headers(&session)?,
            Query(MembershipPage::default())
        )
        .await
        .is_err()
    );
    Ok(())
}
