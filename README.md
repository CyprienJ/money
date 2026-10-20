# Finances perso *(nom provisoire)*

Application de bureau (**Tauri + Rust**, Windows et Linux) de gestion de finances personnelles, inspirée de Microsoft Money 2000.

Son objectif est de **comprendre où va l'argent** : importer ses relevés bancaires, les catégoriser, puis analyser dépenses, revenus et patrimoine avec des statistiques et des graphiques.

---

## Philosophie

- **Analyse a posteriori, pas budget.** Le logiciel décrit ce qui s'est passé. Il ne cherche pas à planifier ni à contraindre les dépenses : pas de budget prévisionnel, pas d'enveloppes, pas d'objectifs.
- **Local et mono-utilisateur.** Les données restent sur la machine, dans un fichier unique. Pas de serveur, pas de compte en ligne, pas de synchronisation.
- **Import d'abord.** Les opérations viennent principalement des relevés de banque (CSV, PDF). La saisie manuelle est un complément.
- **Des statistiques justes avant tout.** Une statistique fausse est pire que pas de statistique. D'où les contrôles à l'import et l'exclusion des virements internes.

---

## Périmètre

### Hors périmètre (décision définitive)

| Fonctionnalité | Raison |
|---|---|
| Rapprochement bancaire / pointage | Remplacé par le contrôle automatique des soldes à l'import |
| Budget prévisionnel, budget par enveloppes | Ce n'est pas un outil de budget |
| Objectifs d'épargne, plan de remboursement de dettes | Idem |
| Multi-utilisateurs / partage foyer | Usage personnel |
| Synchronisation multi-appareils / version mobile | Client lourd local |
| API / scripting / plugins | Pas de besoin identifié |

---

## Roadmap

### V1 — Importer, consulter, visualiser

#### Comptes
- **Plusieurs comptes** : courant, livret, carte à débit différé…
- **Comptes de placement** (PEA, CTO, assurance-vie) : en V1, seule la **valeur totale** est suivie, saisie manuellement à la date voulue. Le détail des titres arrive en V2.
- **Actifs et dettes** : immobilier, véhicule, prêts en cours, avec une valeur ou un capital restant dû saisi manuellement et historisé.

#### Import et saisie
- **Import CSV** avec mapping des colonnes. Le mapping est mémorisé par banque ou par format.
- **Import PDF** des relevés, avec un parser par format de banque. On commence par la banque principale.
- **Contrôle des soldes à l'import** : quand le relevé fournit un solde initial et un solde final, on vérifie que `solde initial + Σ opérations = solde final`. En cas d'écart, l'import est signalé comme suspect et l'utilisateur choisit de l'annuler ou de le corriger.
- **Détection des doublons** : une opération déjà présente (même compte, même date, même montant, même libellé brut) n'est pas réimportée. Les cas ambigus sont présentés à l'utilisateur.
- **Annulation d'un import** : chaque import est un **lot**, et on peut supprimer un lot entier en une action.
- **Sauvegarde automatique avant chaque import** : copie horodatée du fichier de données.
- **Saisie et édition manuelle** des opérations.

#### Catégorisation
- **Catégories et sous-catégories** personnalisables, sur deux niveaux.
- **Catégorie spéciale « Virement interne »**, exclue de toutes les statistiques de revenus et de dépenses. En V1, on l'attribue à la main ou via la reprise automatique.
- **Normalisation simple des libellés** : suppression des dates, numéros de carte, références de prélèvement et préfixes bancaires (`CB`, `PRLV SEPA`, `VIR`…) à l'aide d'expressions régulières. Le libellé brut est toujours conservé.
- **Reprise automatique de catégorie** : à l'import, une opération dont le libellé normalisé a déjà été catégorisé reçoit la même catégorie. L'utilisateur peut corriger.

#### Échéancier
- **Opérations récurrentes** (loyer, salaire, abonnements) avec une périodicité : mensuelle, trimestrielle, annuelle…
- **En V1, l'échéancier sert à l'affichage** : il liste les prochaines échéances sur le tableau de bord, sans créer d'opérations. Les opérations réelles viennent de l'import, ce qui évite tout doublon.

#### Analyse et graphiques
- **Dépenses par catégorie**, en camembert ou en barres, sur une période choisie.
- **Évolution du solde** dans le temps, par compte et tous comptes confondus.
- **Revenus vs dépenses** par mois et par année.
- **Comparaison de périodes** : ce mois contre le mois précédent, ou contre le même mois l'an dernier.
- **Top tiers et plus grosses dépenses**, calculés sur les libellés normalisés.
- **Patrimoine net** dans le temps : liquidités + placements + actifs − dettes.
- **Tableau de bord** d'accueil : soldes des comptes, patrimoine net, prochaines échéances, graphiques résumés du mois.

### V2 — Automatiser et enrichir

#### Catégorisation et tiers
- **Règles de catégorisation configurables**, du type « libellé contient X » ou « montant entre A et B », avec priorités.
- **Gestion des tiers** : fusion de plusieurs libellés normalisés en un seul tiers (alias) et renommage.
- **Appariement automatique des virements internes** : le débit sur le compte A et le crédit correspondant sur le compte B sont liés.

#### Opérations
- **Tags et notes** libres.
- **Pièces jointes** (factures, justificatifs), copiées à côté du fichier de données.
- **Recherche et filtres avancés** : montant, période, tiers, catégorie, tag, compte.

#### Suivi
- **Rapprochement échéancier ↔ import** : une opération importée est associée à l'échéance prévue (même montant à une tolérance près, date à ± N jours), qui passe alors en « réalisée ».
- **Détection automatique des abonnements** : repérage des opérations récurrentes non déclarées dans l'échéancier.
- **Alertes** : solde projeté négatif, dépense inhabituelle pour une catégorie, échéance manquante.
- **Prévision de trésorerie** : solde projeté à partir du solde actuel et de l'échéancier.

#### Placements
- **Portefeuille détaillé** : titres, quantités, prix de revient, mouvements (achat, vente, dividende).
- **Cours de bourse récupérés automatiquement**.
- **Performance et plus-values**, latentes et réalisées.

#### Technique
- **Multi-devises** avec conversion vers la devise de référence.
- **Chiffrement** du fichier de données, protégé par mot de passe.
- **Sauvegarde complète** : sauvegardes planifiées, rotation, restauration depuis l'interface.

### Plus tard

- Import OFX / QIF
- Synchronisation bancaire automatique (agrégateur DSP2)
- Catégorisation par apprentissage / IA
- Opérations ventilées (une opération répartie sur plusieurs catégories)
- Rapports personnalisables exportables (PDF, CSV, Excel)
- Export complet des données dans un format ouvert
- Annuler / rétablir et historique des modifications
- Simulateurs (prêt, épargne, intérêts composés)
- Projection long terme (retraite, gros projets)
- Aide à la déclaration d'impôts
- Assistant IA (questions en langage naturel sur ses finances)

---

## Règles de calcul

- **Les virements internes sont exclus** de tous les indicateurs de revenus, de dépenses, des tops et des comparaisons. Ils restent pris en compte dans les soldes.
- **Le patrimoine net** à une date D se calcule ainsi :
  - Σ soldes des comptes courants et d'épargne à D
  - \+ dernière valorisation connue ≤ D de chaque compte de placement et de chaque actif
  - − dernier capital restant dû connu ≤ D de chaque dette
- **Les montants** sont stockés en **centimes (entiers)**, jamais en virgule flottante.
- **Le signe** est négatif pour un débit et positif pour un crédit, du point de vue du compte.

---

## Modèle de données (V1, avec anticipation des versions suivantes)

Les colonnes notées *(prévu)* sont créées dès la V1, mais ne sont exploitées qu'en V2 ou plus tard. Cela évite des migrations coûteuses.

```text
account
  id, name, type (CHECKING | SAVINGS | CREDIT_CARD | INVESTMENT | ASSET | LIABILITY),
  currency (prévu, default 'EUR'), institution, opening_balance_cents, opening_date, archived

operation
  id, account_id, date, value_date,
  amount_cents, currency (prévu),
  raw_label            -- original bank label, never modified
  normalized_label     -- cleaned label used for categorization and payees
  payee_id (prévu)     -- V2 payee management
  import_batch_id      -- null for manual entries
  is_internal_transfer -- derived from category in V1, explicit pairing in V2
  transfer_pair_id (prévu)
  note (prévu), created_at, updated_at

operation_split          -- one row per operation in V1; several rows once splitting is supported
  id, operation_id, category_id, amount_cents

category
  id, name, parent_id, is_internal_transfer (system flag), is_income

import_batch
  id, account_id, source_type (CSV | PDF), source_file_name, source_file_hash,
  imported_at, statement_start_balance_cents, statement_end_balance_cents,
  balance_check_status (OK | MISMATCH | NOT_AVAILABLE)

csv_mapping
  id, name, delimiter, encoding, date_format, decimal_separator,
  column_date, column_label, column_amount, column_debit, column_credit, skip_lines

label_category_memory    -- V1 automatic category reuse
  normalized_label, category_id, last_used_at

valuation                -- history of value for INVESTMENT / ASSET / LIABILITY accounts
  id, account_id, date, value_cents

scheduled_operation      -- schedule (display only in V1)
  id, account_id, label, amount_cents, category_id,
  frequency (MONTHLY | QUARTERLY | YEARLY | ...), day_of_period, start_date, end_date

-- Prévus (V2+) : payee, payee_alias, tag, operation_tag, attachment,
--               categorization_rule, security, security_price, investment_transaction
```

Note : la catégorie est portée par `operation_split` dès la V1. Ainsi, l'arrivée de la ventilation ne change aucune requête de statistique.

---

## Flux d'import

```text
File (CSV / PDF)
  → Parser (CSV mapping or bank-specific PDF parser)
  → Raw lines (date, label, amount) + statement balances when available
  → Balance check (start + Σ = end ?)          → warning if mismatch
  → Label normalization (regex)
  → Duplicate detection                         → ambiguous cases shown to the user
  → Automatic category reuse (label memory)
  → Preview screen (user validates / corrects)
  → Automatic backup of the data file
  → Insert as a new import_batch
```

Un import peut être annulé à tout moment en supprimant son lot.

---

## Ordre de développement V1

1. **Socle** : modèle de données, gestion des comptes, saisie manuelle des opérations
2. **Import CSV** : mapping des colonnes, détection des doublons, lots d'import et annulation, sauvegarde avant import
3. **Catégorisation** : catégories, virement interne, normalisation des libellés, reprise automatique de catégorie
4. **Premiers graphiques** : dépenses par catégorie, évolution du solde, revenus vs dépenses
5. **Import PDF** : parser pour la banque principale et contrôle des soldes
6. **Patrimoine** : comptes de placement, actifs, dettes, valorisations, patrimoine net
7. **Finitions** : échéancier (affichage), comparaison de périodes, top tiers, tableau de bord

L'import CSV passe avant le PDF : il fournit rapidement des données réelles pour construire tout le reste, et le parser PDF produira ensuite la même structure de lignes brutes.

---

## Stack technique

### Contraintes

- Utilisable sous **Windows et Linux**
- **Tout tourne en local** : aucun service distant, aucune donnée qui sort de la machine
- Base de données locale en fichier unique
- Graphiques interactifs riches (c'est le cœur de la V1)
- Extraction de texte depuis des relevés PDF
- Ouverture vers l'IA locale dans les versions ultérieures

### Décision : Tauri 2 + Rust

L'application est une **application de bureau Tauri**. L'interface est écrite en Vue/TypeScript et rendue par le moteur web du système. La logique métier et l'accès aux données sont en **Rust**. Le front appelle le backend via l'IPC de Tauri (des *commandes*), sans serveur HTTP ni port ouvert.

| Couche | Choix | Rôle / remarque |
|---|---|---|
| Framework applicatif | **Tauri 2** | Fenêtre native, IPC, permissions par commande (*capabilities*), bundler |
| Langage backend | **Rust** | Logique métier, import, normalisation, statistiques |
| Base de données | **SQLite** via **sqlx** | Requêtes vérifiées à la compilation ; un seul fichier de données |
| Migrations | **sqlx migrate** | Fichiers SQL versionnés, appliqués au démarrage |
| Montants | `i64` en centimes | Jamais de flottant ; `rust_decimal` si des calculs de taux l'exigent |
| Dates | **chrono** (ou **time**) | |
| Erreurs | **thiserror** (domaine) / **anyhow** (outillage) | Erreurs typées remontées au front |
| Import CSV | crate **csv** + **encoding_rs** | Séparateur `;`, encodage ISO-8859-1 / Windows-1252 courants |
| Import PDF | **pdfium-render** | Texte avec positions via PDFium ; bibliothèque PDFium embarquée dans le bundle |
| Normalisation des libellés | crate **regex** | |
| Frontend | **Vue 3 + TypeScript + Vite** | |
| Graphiques | **Apache ECharts** (via **vue-echarts**) | Camemberts, séries temporelles, zoom sur une période |
| Typage front ↔ back | **tauri-specta** | Génère les types et fonctions TypeScript à partir des commandes Rust |
| Accès système | **tauri-plugin-dialog**, **tauri-plugin-fs** | Sélection des fichiers de relevés |
| Tests | `cargo test` (back), **Vitest** (front) | Relevés réels anonymisés comme fixtures d'import |
| Packaging | Bundler Tauri | `.msi` / NSIS (Windows), `.deb`, `.rpm`, AppImage (Linux) |
| CI | **GitHub Actions** + **tauri-action** | Matrice `windows-latest` / `ubuntu-latest` |

#### Pistes pour l'IA (versions ultérieures)

- **Catégorisation** : classifieur local (TF-IDF + régression logistique avec **linfa**), ou embeddings (**fastembed-rs** / **ort**, c'est-à-dire ONNX Runtime) + plus proches voisins sur les opérations déjà catégorisées.
- **Assistant** : LLM local via **Ollama** (crate **ollama-rs**). Le modèle appelle les fonctions de statistiques existantes (*tool calling*) au lieu de générer du SQL.
- L'IA reste **optionnelle** : elle est activée seulement si Ollama est détecté sur la machine.

#### Points d'attention

- **Moteur web différent selon l'OS** : WebView2 (Chromium) sous Windows, **WebKitGTK** sous Linux. Il faut tester le rendu et les performances d'ECharts sur les deux plateformes dès le début.
- **Dépendances Linux** : WebKitGTK est déclaré en dépendance du `.deb` / `.rpm` et embarqué dans l'AppImage.
- **Permissions Tauri 2** : chaque commande et chaque accès fichier doit être autorisé explicitement dans `src-tauri/capabilities/`.
- **Emplacement des données** : `app_data_dir()` de Tauri, c'est-à-dire `%APPDATA%\<app>` sous Windows et `~/.local/share/<app>` sous Linux. Les sauvegardes automatiques sont rangées dans un sous-dossier `backups/`.
- **Chiffrement (V2)** : passer à SQLCipher (sqlx / libsqlite3-sys avec la fonctionnalité correspondante) sans impact sur le reste du code.

### Structure du projet

```text
finances-perso/
├── src/                          # Frontend Vue 3 + TypeScript
│   ├── bindings.ts               # generated by tauri-specta, do not edit
│   ├── views/                    # dashboard, accounts, operations, import, charts
│   ├── components/
│   └── charts/                   # ECharts option builders
├── src-tauri/                    # Backend Rust
│   ├── migrations/               # sqlx SQL migrations
│   ├── capabilities/             # Tauri 2 permissions
│   └── src/
│       ├── main.rs
│       ├── commands/             # #[tauri::command] entry points, thin layer
│       ├── db/                   # pool, repositories
│       ├── account/              # accounts, valuations, net worth
│       ├── operation/            # operations, splits, categories
│       ├── importing/            # import batches, duplicates, balance check, backup
│       │   ├── csv/              # CSV parser + column mappings
│       │   └── pdf/              # StatementParser trait, one impl per bank
│       ├── labeling/             # label normalization, category memory
│       └── stats/                # aggregation queries for charts (future AI tools)
├── tests/fixtures/               # anonymized real statements + expected results
└── .github/workflows/            # tauri-action build matrix
```

### Alternatives étudiées

| | Spring Boot + navigateur | **Tauri + Rust** (retenu) | Tauri + TypeScript |
|---|---|---|---|
| Langages | Java + TS | Rust + TS | TS |
| Courbe d'apprentissage | Faible | Forte | Faible |
| Taille de l'exécutable | ~80–120 Mo (JRE embarquée) | ~10 Mo | ~10 Mo |
| Fenêtre native | Non (navigateur) | Oui | Oui |
| Port local exposé | Oui (à sécuriser) | Non | Non |
| Outillage IA | Excellent (Spring AI, LangChain4j) | Correct | Bon |
| Séparation métier / UI | Naturelle | Naturelle | À discipliner |

Écartées :
- **JavaFX** : graphiques intégrés trop limités, et UI riche plus longue à construire.
- **Electron** : environ 150 Mo (Chromium embarqué) et backend en Node.
- **Tauri + backend Java en sidecar** : cumule le port local exposé et le poids de la JRE, sans avantage réel sur Spring Boot seul.

---

## Questions ouvertes

- Liste des banques et formats de relevés à supporter en premier
- Profondeur de l'historique à importer au départ
- Arborescence de catégories par défaut proposée au premier lancement
