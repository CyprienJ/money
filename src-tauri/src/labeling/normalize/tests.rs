use chrono::NaiveDate;

use super::*;
use crate::account::{self, AccountType, NewAccount};
use crate::db::tests::memory_pool;
use crate::labeling::memory::recall;
use crate::operation::{self, NewOperation, category};

fn check(cases: &[(&str, &str)]) {
    for (raw, expected) in cases {
        assert_eq!(normalize_label(raw), *expected, "raw label: {raw:?}");
    }
}

#[test]
fn removes_card_payment_prefixes_dates_and_card_numbers() {
    check(&[
        ("CB CARREFOUR MKT 18/10 CARTE 4417", "CARREFOUR MKT"),
        ("CB CARREFOUR MKT 18/10 CARTE 4417 ", "CARREFOUR MKT"),
        ("CB LE PETIT BISTROT 17/10", "LE PETIT BISTROT"),
        ("CB FNAC.COM 13/10", "FNAC.COM"),
        (
            "CARTE 07/09 MAXICOFFEE SUD SATURNE",
            "MAXICOFFEE SUD SATURNE",
        ),
        ("CARTE X1234 SNCF INTERNET", "SNCF INTERNET"),
        ("CB Boulangerie Pâtisserie 06/09", "BOULANGERIE PÂTISSERIE"),
        (
            "PAIEMENT PAR CARTE 4417****1234 AMAZON EU 12.10.2026",
            "AMAZON EU",
        ),
        ("PAIEMENT CB DU 03/10/26 LECLERC", "LECLERC"),
        ("ACHAT CB TOTALENERGIES 21/09 XXXX4417", "TOTALENERGIES"),
    ]);
}

#[test]
fn removes_dates_in_words() {
    check(&[
        (
            "Intérêts nets versés pour Compte d&#39;épargne le Apr 5, 2025",
            "INTÉRÊTS NETS VERSÉS POUR COMPTE D'ÉPARGNE",
        ),
        (
            "Intérêts nets versés pour Compte d&#39;épargne le May 5, 2025",
            "INTÉRÊTS NETS VERSÉS POUR COMPTE D'ÉPARGNE",
        ),
        (
            "Interest payment on September 30th, 2025",
            "INTEREST PAYMENT",
        ),
        ("Cotisation carte du 5 avr. 2025", "COTISATION CARTE"),
        ("Cotisation carte du 1er février 2025", "COTISATION CARTE"),
        ("LOYER AVRIL 2025", "LOYER"),
        ("Loyer de décembre 2025", "LOYER"),
        // A month without a day nor a year is kept: it can be part of a name.
        (
            "VIR SEPA RECU /DE EMPLOYEUR SA; SALAIRE SEPT",
            "EMPLOYEUR SA SALAIRE SEPT",
        ),
        ("CB MARS DISTRIBUTION 18/10", "MARS DISTRIBUTION"),
    ]);
}

#[test]
fn decodes_html_entities() {
    check(&[
        ("CB H&amp;M 18/10", "H&M"),
        ("L&#x27;ATELIER", "L'ATELIER"),
        // Unknown entities are left as they are (the ";" separator is then dropped).
        ("AT&T &foo;", "AT&T &FOO"),
    ]);
}

#[test]
fn removes_direct_debit_and_transfer_prefixes() {
    check(&[
        ("PRLV SEPA SPOTIFY AB", "SPOTIFY AB"),
        ("PRLV SEPA CREDIT IMMO ECH 10/26", "CREDIT IMMO"),
        (
            "PRLV SEPA EDF CLIENTS PARTICULIERS",
            "EDF CLIENTS PARTICULIERS",
        ),
        (
            "PRELEVEMENT EUROPEEN FREE MOBILE REF: 2026FM123456",
            "FREE MOBILE",
        ),
        (
            "PRLV SEPA LOYER SCI DES ÉRABLES RUM ABC123XYZ",
            "LOYER SCI DES ÉRABLES",
        ),
        ("VIR SEPA VERS LIVRET A", "VERS LIVRET A"),
        ("VIR VERS LIVRET A", "VERS LIVRET A"),
        (
            "VIR SEPA RECU /DE EMPLOYEUR SA; SALAIRE SEPT",
            "EMPLOYEUR SA SALAIRE SEPT",
        ),
        ("VIR INST Captain Hadock", "CAPTAIN HADOCK"),
        ("VIREMENT SEPA EMIS VERS M DUPONT", "VERS M DUPONT"),
        ("Virement Versement PEA", "VERSEMENT PEA"),
        ("REMISE CHQ 1234567", "REMISE CHQ 1234567"),
        ("CHQ 1234567", "CHQ 1234567"),
    ]);
}

#[test]
fn removes_ibans_and_references() {
    check(&[
        (
            "Incoming transfer from Captain Hadock (FR1234567891234567891234567)",
            "INCOMING TRANSFER FROM CAPTAIN HADOCK",
        ),
        (
            "VIR INST Captain Hadock Envoye depuis Revolut 1234567891234567891",
            "CAPTAIN HADOCK ENVOYE DEPUIS REVOLUT",
        ),
        (
            "VIR SEPA M DUPONT FR76 1451 8000 0000 0000 0000 000",
            "M DUPONT",
        ),
        (
            "VIR INST REMBOURSEMENT 26267IP1RT501085988",
            "REMBOURSEMENT",
        ),
        (
            "PRLV SEPA DGFIP IMPOT 1234567890 ID EMETTEUR FR46ZZZ005002",
            "DGFIP IMPOT",
        ),
    ]);
}

#[test]
fn keeps_meaningful_labels() {
    check(&[
        ("REMBOURSEMENT SÉCU CPAM", "REMBOURSEMENT SÉCU CPAM"),
        ("INTERETS CREDITEURS", "INTERETS CREDITEURS"),
        ("Intérêts Interest payment", "INTÉRÊTS INTEREST PAYMENT"),
        ("RETRAIT DAB 12/10 PARIS 11", "RETRAIT DAB PARIS 11"),
        ("  Restaurant   avec Paul ", "RESTAURANT AVEC PAUL"),
        ("Pharmacie du Centre", "PHARMACIE DU CENTRE"),
        ("CARTEFOUR", "CARTEFOUR"),
        ("CBD SHOP", "CBD SHOP"),
        ("VIRGIN MOBILE", "VIRGIN MOBILE"),
    ]);
}

#[test]
fn falls_back_to_the_raw_label_when_nothing_remains() {
    check(&[
        ("CB 18/10", "CB 18/10"),
        ("PRLV SEPA", "PRLV SEPA"),
        ("  vir  ", "VIR"),
    ]);
}

#[test]
fn renormalizes_stored_labels() {
    tauri::async_runtime::block_on(async {
        let pool = memory_pool().await;
        let account_id = account::create(
            &pool,
            NewAccount {
                name: "Courant".to_owned(),
                account_type: AccountType::Checking,
                institution: None,
                opening_balance_cents: 0,
                opening_date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            },
        )
        .await
        .unwrap()
        .id;
        let created = operation::create(
            &pool,
            NewOperation {
                account_id,
                date: NaiveDate::from_ymd_opt(2026, 9, 1).unwrap(),
                value_date: None,
                amount_cents: -1000,
                label: "CB CARREFOUR MKT 18/10".to_owned(),
                category_id: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(created.normalized_label, "CARREFOUR MKT");

        // A label normalized by older rules.
        sqlx::query!(
            "UPDATE operation SET normalized_label = 'OLD' WHERE id = ?1",
            created.id
        )
        .execute(&pool)
        .await
        .unwrap();
        // Categories remembered under the old label (recent) and the new one (older).
        let categories = category::list(&pool).await.unwrap();
        let id = |name: &str| categories.iter().find(|c| c.name == name).unwrap().id;
        let (groceries, health) = (id("Courses"), id("Santé"));
        sqlx::query!(
            "INSERT INTO label_category_memory (normalized_label, category_id, last_used_at)
             VALUES ('OLD', ?1, '2026-10-02T10:00:00Z'), ('CARREFOUR MKT', ?2, '2026-10-01T10:00:00Z')",
            groceries,
            health
        )
        .execute(&pool)
        .await
        .unwrap();

        assert_eq!(renormalize_all(&pool).await.unwrap(), 1);
        let operation = operation::get(&pool, created.id).await.unwrap();
        assert_eq!(operation.normalized_label, "CARREFOUR MKT");
        assert_eq!(operation.raw_label, "CB CARREFOUR MKT 18/10");
        let mut conn = pool.acquire().await.unwrap();
        let remembered = recall(&mut conn, &["CARREFOUR MKT"]).await.unwrap();
        assert_eq!(remembered["CARREFOUR MKT"], groceries);
        drop(conn);
        assert_eq!(renormalize_all(&pool).await.unwrap(), 0);
    });
}
