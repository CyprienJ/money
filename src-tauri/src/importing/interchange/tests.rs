use chrono::NaiveDate;

use super::*;
use crate::importing::{RawLine, Statement};

fn date(text: &str) -> NaiveDate {
    text.parse().unwrap()
}

fn line(day: &str, label: &str, amount_cents: i64) -> RawLine {
    RawLine {
        date: date(day),
        value_date: None,
        label: label.to_owned(),
        amount_cents,
        note: None,
        source_category: None,
        source_page: None,
        source_line: None,
    }
}

#[test]
fn reads_amounts_without_floating_point() {
    assert_eq!(parse_plain_amount("-12.34"), Some(-1234));
    assert_eq!(parse_plain_amount("-12,3"), Some(-1230));
    assert_eq!(parse_plain_amount("+1500"), Some(150_000));
    assert_eq!(parse_plain_amount("0.125"), Some(13));
    assert_eq!(parse_plain_amount("12a"), None);

    assert_eq!(parse_grouped_amount("-1,234.56"), Some(-123_456));
    assert_eq!(parse_grouped_amount("-1 234,56"), Some(-123_456));
    assert_eq!(parse_grouped_amount("1.234,5"), Some(123_450));
    assert_eq!(parse_grouped_amount("−8,40"), Some(-840));
    assert_eq!(parse_grouped_amount("1,000"), Some(100_000));
    assert_eq!(parse_grouped_amount("12"), Some(1200));
    assert_eq!(parse_grouped_amount(""), None);
}

#[test]
fn decodes_utf8_or_windows_1252() {
    assert_eq!(decode(b"\xEF\xBB\xBFSoci\xC3\xA9t\xC3\xA9"), "Société");
    assert_eq!(decode(b"Soci\xE9t\xE9"), "Société");
}

const OFX_SGML: &str = "OFXHEADER:100\r
DATA:OFXSGML\r
VERSION:102\r
CHARSET:1252\r
\r
<OFX>
<SIGNONMSGSRSV1><SONRS><STATUS><CODE>0<SEVERITY>INFO</STATUS><DTSERVER>20261020<LANGUAGE>FRA</SONRS></SIGNONMSGSRSV1>
<BANKMSGSRSV1><STMTTRNRS><TRNUID>1<STMTRS>
<CURDEF>EUR
<BANKACCTFROM><BANKID>12345<ACCTID>0001<ACCTTYPE>CHECKING</BANKACCTFROM>
<BANKTRANLIST><DTSTART>20261001<DTEND>20261020
<STMTTRN><TRNTYPE>DEBIT<DTPOSTED>20261005120000[+1:CET]<TRNAMT>-64.28<FITID>A1<NAME>CB CARREFOUR MKT<MEMO>CARTE 4974 04/10
</STMTTRN>
<STMTTRN><TRNTYPE>CREDIT<DTPOSTED>20261015<TRNAMT>3280,00<FITID>A2<NAME>VIR SALAIRE &amp; PRIMES
</STMTTRN>
<STMTTRN><TRNTYPE>OTHER<DTPOSTED>20261016<TRNAMT>0.00<FITID>A3<NAME>INFO
</STMTTRN>
<STMTTRN><TRNTYPE>FEE<DTPOSTED>20261018<TRNAMT>-2.50<FITID>A4<MEMO>FRAIS TENUE
</STMTTRN>
</BANKTRANLIST>
<LEDGERBAL><BALAMT>4213.22<DTASOF>20261020</LEDGERBAL>
<AVAILBAL><BALAMT>4000.00<DTASOF>20261020</AVAILBAL>
</STMTRS></STMTTRNRS></BANKMSGSRSV1>
</OFX>
";

#[test]
fn reads_an_ofx_1_statement() {
    let read = ofx::parse(OFX_SGML.as_bytes()).unwrap();
    assert_eq!(read.currency.as_deref(), Some("EUR"));
    let statement = read.statement;
    // Line of the <STMTTRN> of each transaction.
    let positions: Vec<Option<u32>> = statement.lines.iter().map(|l| l.source_line).collect();
    assert_eq!(positions, [Some(12), Some(14), Some(18)]);
    assert_eq!(
        crate::importing::unplaced(statement.lines),
        [
            RawLine {
                note: Some("CARTE 4974 04/10".to_owned()),
                ..line("2026-10-05", "CB CARREFOUR MKT", -6428)
            },
            line("2026-10-15", "VIR SALAIRE & PRIMES", 328_000),
            line("2026-10-18", "FRAIS TENUE", -250),
        ]
    );
    assert_eq!(statement.end_balance_cents, Some(421_322));
    assert_eq!(statement.start_balance_cents, None);
    assert_eq!(
        statement.period,
        Some((date("2026-10-01"), date("2026-10-20")))
    );
}

#[test]
fn reads_an_ofx_2_statement() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<?OFX OFXHEADER="200" VERSION="220"?>
<OFX><CREDITCARDMSGSRSV1><CCSTMTTRNRS><CCSTMTRS>
  <CURDEF>USD</CURDEF>
  <BANKTRANLIST>
    <STMTTRN><TRNTYPE>DEBIT</TRNTYPE><DTPOSTED>20261003</DTPOSTED><TRNAMT>-19.99</TRNAMT>
      <PAYEE><NAME>Café &#233;toile</NAME></PAYEE><MEMO>Café &#233;toile</MEMO></STMTTRN>
  </BANKTRANLIST>
</CCSTMTRS></CCSTMTTRNRS></CREDITCARDMSGSRSV1></OFX>"#;
    let read = ofx::parse(xml.as_bytes()).unwrap();
    assert_eq!(read.currency.as_deref(), Some("USD"));
    // The memo repeating the name is not kept.
    assert_eq!(
        crate::importing::unplaced(read.statement.lines),
        [line("2026-10-03", "Café étoile", -1999)]
    );
}

#[test]
fn refuses_unsupported_ofx_files() {
    assert_eq!(
        ofx::parse(b"Date;Libelle;Montant"),
        Err(InterchangeError::NotOfx)
    );
    let investment = "<OFX><INVSTMTMSGSRSV1><INVSTMTTRNRS><INVSTMTRS><CURDEF>EUR</INVSTMTRS></INVSTMTTRNRS></INVSTMTMSGSRSV1></OFX>";
    assert_eq!(
        ofx::parse(investment.as_bytes()),
        Err(InterchangeError::InvestmentStatement)
    );
    let two = "<OFX><STMTRS><CURDEF>EUR</STMTRS><STMTRS><CURDEF>EUR</STMTRS></OFX>";
    assert_eq!(
        ofx::parse(two.as_bytes()),
        Err(InterchangeError::SeveralAccounts(2))
    );
    let bad_date =
        "<OFX>\n<STMTRS>\n<STMTTRN><DTPOSTED>2026-10<TRNAMT>-1.00<NAME>X</STMTTRN></STMTRS></OFX>";
    assert_eq!(
        ofx::parse(bad_date.as_bytes()),
        Err(InterchangeError::InvalidDate {
            line: 3,
            text: "2026-10".to_owned()
        })
    );
    let no_amount = "<OFX>\n<STMTRS>\n<STMTTRN><DTPOSTED>20261001<NAME>X</STMTTRN></STMTRS></OFX>";
    assert_eq!(
        ofx::parse(no_amount.as_bytes()),
        Err(InterchangeError::IncompleteTransaction { line: 3 })
    );
}

/// Export of an account by the French version of Money 2000, in Windows-1252.
fn money_qif() -> Vec<u8> {
    let text = "!Type:Bank
D01/01/99
T1 500,00
CX
PSolde d'ouverture
L[Compte courant]
^
D15/01/99
T-1 234,56
PSociété Générale
MÉchéance prêt
LLogement:Crédit immobilier
^
D20/01/99
T-300,00
PVirement épargne
L[Livret A]
^
D31/12' 0
T-20,00
PCarrefour
SAlimentation:Courses
$-12,00
SLoisirs/Vacances
$-8,00
^
D02/01'01
T-45,50
PFnac
SLoisirs
$-40,00
SLoisirs
$-5,50
^
D03/01'01
T0,00
PChèque annulé
^
D04/01'01
T-9,90
MAbonnement
^
";
    encoding_rs::WINDOWS_1252.encode(text).0.into_owned()
}

#[test]
fn reads_a_money_qif_export() {
    let statement = qif::parse(&money_qif()).unwrap();
    // First line of each record.
    let positions: Vec<Option<u32>> = statement.lines.iter().map(|l| l.source_line).collect();
    assert_eq!(positions, [8, 14, 19, 27, 39].map(Some));
    let statement = Statement {
        lines: crate::importing::unplaced(statement.lines),
        ..statement
    };
    assert_eq!(statement.start_balance_cents, Some(150_000));
    assert_eq!(statement.end_balance_cents, None);
    let with = |line: RawLine, category: Option<&str>, note: Option<&str>| RawLine {
        source_category: category.map(str::to_owned),
        note: note.map(str::to_owned),
        ..line
    };
    assert_eq!(
        statement.lines,
        [
            with(
                line("1999-01-15", "Société Générale", -123_456),
                Some("Logement:Crédit immobilier"),
                Some("Échéance prêt")
            ),
            with(
                line("1999-01-20", "Virement épargne", -30_000),
                Some("[Livret A]"),
                None
            ),
            with(
                line("2000-12-31", "Carrefour", -2000),
                None,
                Some("Ventilation : Alimentation:Courses -12,00 ; Loisirs -8,00")
            ),
            with(
                line("2001-01-02", "Fnac", -4550),
                Some("Loisirs"),
                Some("Ventilation : Loisirs -40,00 ; Loisirs -5,50")
            ),
            // Without a payee, the memo is the label.
            line("2001-01-04", "Abonnement", -990),
        ]
    );
}

#[test]
fn deduces_the_order_of_the_qif_dates() {
    let us = "!Type:CCard\nD12/31/1999\nT-1.00\nPA\n^\nD1/ 2/00\nT-2.00\nPB\n^\n";
    let lines = qif::parse(us.as_bytes()).unwrap().lines;
    assert_eq!(lines[0].date, date("1999-12-31"));
    assert_eq!(lines[1].date, date("2000-01-02"));

    let iso = "!Type:Bank\nD2026-10-20\nT-1.00\nPA\n^\n";
    assert_eq!(
        qif::parse(iso.as_bytes()).unwrap().lines[0].date,
        date("2026-10-20")
    );
}

#[test]
fn refuses_unsupported_qif_files() {
    assert_eq!(qif::parse(b"Date;Montant"), Err(InterchangeError::NotQif));
    assert_eq!(
        qif::parse(b"!Type:Cat\nNAlimentation\nE\n^\n"),
        Err(InterchangeError::NoAccount)
    );
    assert_eq!(
        qif::parse(b"!Type:Invst\nD01/02/26\nNBuy\nYCW8\nT100.00\n^\n"),
        Err(InterchangeError::InvestmentStatement)
    );
    let two = "!Option:AutoSwitch\n!Account\nNCourant\nTBank\n^\nNLivret\nTBank\n^\n!Clear:AutoSwitch\n\
               !Account\nNCourant\n^\n!Type:Bank\nD01/02/26\nT-1,00\nPA\n^\n\
               !Account\nNLivret\n^\n!Type:Bank\nD01/02/26\nT1,00\nPB\n^\n";
    assert_eq!(
        qif::parse(two.as_bytes()),
        Err(InterchangeError::SeveralAccounts(2))
    );
    assert_eq!(
        qif::parse(b"!Type:Bank\nD01/02/26\nPA\n^\n"),
        Err(InterchangeError::IncompleteTransaction { line: 2 })
    );
    assert_eq!(
        qif::parse(b"!Type:Bank\nD31/31/26\nT-1,00\n^\n"),
        Err(InterchangeError::InvalidDate {
            line: 2,
            text: "31/31/26".to_owned()
        })
    );
    assert_eq!(
        qif::parse(b"!Type:Bank\nD01/02/26\nTabc\n^\n"),
        Err(InterchangeError::InvalidAmount {
            line: 3,
            text: "abc".to_owned()
        })
    );
}
