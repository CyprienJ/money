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
- **Import CSV** avec mapping des colonnes. Le mapping est mémorisé par banque ou par format ; pour un nouveau fichier, il est détecté automatiquement (encodage, séparateur, colonnes, formats) puis vérifié par l'utilisateur.
- **Import PDF** des relevés, avec un parser par banque. La liste des banques prises en charge est affichée dans l'application. L'objectif à terme est de couvrir la majorité des banques françaises : un parser est ajouté dès qu'un relevé d'exemple anonymisé est disponible. En V1 : **Fortuneo** et **Trade Republic** (pour Trade Republic, seul le compte courant est importé : la partie PEA du relevé relève des comptes de placement, suivis par leurs valorisations).
- **Contrôle des soldes à l'import** : quand le relevé fournit un solde initial et un solde final, on vérifie que `solde initial + Σ opérations = solde final`. En cas d'écart, l'import est signalé comme suspect et l'utilisateur choisit de l'annuler ou de l'importer quand même ; il corrige ensuite en saisissant à la main l'opération manquante. Les lignes lues ne sont pas modifiables dans l'aperçu, et aucune ligne d'ajustement n'est créée automatiquement.
- **Détection des doublons** : une opération déjà présente (même compte, même date, même montant, même libellé brut) n'est pas réimportée. Les cas ambigus sont présentés à l'utilisateur. Un cas est ambigu quand la même opération figure plus de fois dans le fichier que dans le compte, quand une opération du même jour a le même montant mais un autre libellé, ou quand une saisie manuelle a le même montant à ±3 jours. Les lignes ambiguës ne sont pas importées par défaut.
- **Annulation d'un import** : chaque import est un **lot**, et on peut supprimer un lot entier en une action.
- **Sauvegarde automatique avant chaque import** : copie horodatée du fichier de données.
- **Saisie et édition manuelle** des opérations.

#### Catégorisation
- **Catégories et sous-catégories** personnalisables, sur deux niveaux. Une sous-catégorie hérite de la couleur et de l'indicateur « revenu » de sa catégorie principale. Une catégorie qui a des sous-catégories ne peut pas être supprimée ; les opérations d'une catégorie supprimée passent à une catégorie choisie ou redeviennent « à catégoriser ».
- **Arborescence par défaut** créée au premier lancement, modifiable ensuite : Logement (loyer ou crédit, charges et énergie, assurance habitation, travaux et équipement), Courses (alimentation, maison et hygiène), Restaurants (restaurants, cafés et snacks, livraison), Transport (carburant, transports en commun, train et avion, entretien du véhicule, péages et parking), Loisirs (sorties et culture, sport, voyages, shopping, cadeaux), Abonnements (téléphone et internet, streaming, logiciels et presse), Santé (médecin, pharmacie, mutuelle), Revenus (salaire, remboursements, intérêts, autres revenus), Autres (impôts et taxes, frais bancaires, retraits d'espèces, divers).
- **Catégorie spéciale « Virement interne »**, exclue de toutes les statistiques de revenus et de dépenses. En V1, on l'attribue à la main, via la reprise automatique ou via une règle. Elle ne peut être ni modifiée, ni supprimée, ni avoir de sous-catégories.
- **Normalisation simple des libellés** : décodage des entités HTML (`&#39;`), suppression des dates (chiffrées ou en toutes lettres, en français et en anglais : « Apr 5, 2025 », « 5 avr. 2025 »), numéros de carte, références de prélèvement et préfixes bancaires (`CB`, `PRLV SEPA`, `VIR`…) à l'aide d'expressions régulières. Le libellé brut est toujours conservé. Le libellé normalisé, en majuscules, sert de clé ; il est recalculé au démarrage quand les règles évoluent. À l'écran, il est affiché avec une capitale initiale par mot (« Carrefour Mkt ») ; les noms lisibles des tiers viendront en V2.
- **Reprise automatique de catégorie** : à l'import, une opération dont le libellé normalisé a déjà été catégorisé reçoit la même catégorie. L'utilisateur peut corriger.
- **Propagation de la catégorie** : dans l'aperçu d'import, une catégorie choisie sur une ligne s'applique aux autres lignes du même libellé normalisé, sauf celles choisies à la main. Dans l'écran Opérations, catégoriser une opération propose (sans l'imposer) de classer les autres opérations du même libellé encore à catégoriser, et avertit si d'autres opérations du même libellé sont dans une autre catégorie.
- **Règles de catégorisation** *(avancées de la V2, décision du 21/10/2026)* : « libellé normalisé contient X », sans tenir compte des majuscules ni des accents, vers une catégorie. Pas d'autre condition. La première règle qui correspond, dans l'ordre de la liste, l'emporte ; une règle passe avant la reprise automatique. Une règle créée peut être appliquée aux opérations existantes encore à catégoriser.

#### Échéancier
- **Opérations récurrentes** (loyer, salaire, abonnements) avec une périodicité : hebdomadaire, mensuelle, trimestrielle, semestrielle ou annuelle. Le jour est le jour de la semaine pour une échéance hebdomadaire, le jour du mois sinon (ramené au dernier jour des mois plus courts) ; les périodicités de plus d'un mois se répètent à partir du mois de la date de début.
- **En V1, l'échéancier sert à l'affichage** : il liste les prochaines échéances sur le tableau de bord, sans créer d'opérations. Les opérations réelles viennent de l'import, ce qui évite tout doublon.

#### Analyse et graphiques
- **Dépenses par catégorie**, en camembert ou en barres, sur une période choisie.
- **Évolution du solde** dans le temps, par compte et tous comptes confondus.
- **Revenus vs dépenses** par mois et par année.
- **Comparaison de périodes** : ce mois contre le mois précédent, ou contre le même mois l'an dernier.
- **Top tiers et plus grosses dépenses**, calculés sur les libellés normalisés, sur toutes les dépenses (remboursements de crédit compris ; virements internes et revenus exclus).
- **Patrimoine net** dans le temps : liquidités + placements + actifs − dettes.
- **Tableau de bord** d'accueil : soldes des comptes, patrimoine net, prochaines échéances, graphiques résumés du mois.

### V2 — Automatiser et enrichir

#### Catégorisation et tiers
- **Suggestion de catégorie par classifieur local** : un classifieur (TF-IDF + régression logistique, crate **linfa**) apprend sur les opérations déjà catégorisées et propose une catégorie, avec un indice de confiance, pour les libellés qu'aucune règle ni la mémoire des libellés ne couvrent. Il tourne entièrement en local, sans réseau ni modèle téléchargé. Priorité : règle, puis mémoire, puis suggestion ; une suggestion n'est jamais appliquée sans validation. *(Décisions du 22/10/2026)* Un exemple par libellé normalisé distinct et sens du montant, avec sa catégorie la plus fréquente ; pas de classifieur avant 30 libellés et 2 catégories, pas de suggestion sous 50 % de confiance. Le modèle est entraîné au démarrage puis en tâche de fond quand les opérations catégorisées changent. Les suggestions apparaissent dans l'aperçu d'import (✓ par ligne, « Accepter les N suggestions ») et sous la catégorie des opérations à catégoriser (✓, pré-sélection dans la fiche) ; une suggestion acceptée est un choix, retenu pour le libellé.
- **Gestion des tiers** : fusion de plusieurs libellés normalisés en un seul tiers (alias) et renommage.
- **Appariement automatique des virements internes** : le débit sur le compte A et le crédit correspondant sur le compte B sont liés *(décisions du 21/10/2026)*. Candidats : comptes différents, montants opposés, au plus 3 jours d'écart, deux opérations non liées, à catégoriser ou en Virement interne, et pas déjà ignorées. Un appariement est sûr quand chaque opération est le seul candidat de l'autre : il est lié automatiquement après chaque import (et à la demande sur tout l'historique) ; les autres sont proposés dans l'écran Opérations (Lier / Ignorer). Lier deux opérations leur donne la catégorie « Virement interne », sans toucher à la mémoire des libellés : l'exclusion des statistiques reste dérivée de la catégorie. Quitter cette catégorie délie les deux opérations ; « Délier » garde leur catégorie. La fiche d'une opération montre son pendant et permet de la lier à la main (n'importe quelle catégorie, à 15 jours près).

#### Opérations
- **Tags et notes** libres *(décisions du 21/10/2026)* : un tag se crée en le tapant sur une opération, sans tenir compte des majuscules pour retrouver un tag existant, et se renomme ou se supprime depuis l'écran Catégories. Les tags n'ont pas de couleur (pastille neutre). Tags et note s'affichent sous le libellé dans la liste des opérations et sont couverts par la recherche libre ; ils sont modifiables sur toutes les opérations, importées comprises.
- **Pièces jointes** (factures, justificatifs), copiées à côté du fichier de données *(décisions du 21/10/2026)* : 25 Mo au plus par fichier, ouverture avec l'application par défaut du système. Les fichiers d'une opération supprimée ou d'un import annulé sont supprimés avec elle ; les fichiers orphelins sont aussi nettoyés au démarrage. La sauvegarde avant import ne copie que le fichier de données : les pièces jointes relèvent de la sauvegarde complète.
- **Recherche et filtres avancés** : montant, période, tiers, catégorie, tag, compte *(décisions du 21/10/2026)*. Dans l'écran Opérations : un onglet « Tous les comptes » (comptes archivés exclus, colonne Compte à la place du solde), une période personnalisée en plus des périodes prédéfinies, et un panneau « Plus de filtres » : sens (dépenses, recettes) et fourchette de montant en valeur absolue, tiers, tag. Une seule valeur par filtre ; les filtres se combinent (ET). Une catégorie principale inclut ses sous-catégories.

#### Suivi
- **Rapprochement échéancier ↔ import** : une opération importée est associée à l'échéance prévue (même montant à une tolérance près, date à ± N jours), qui passe alors en « réalisée » *(décisions du 21/10/2026)*. Candidats : même compte, même sens, montant à ± 10 % de l'échéance, date à ± 5 jours de l'occurrence, opération pas encore associée, proposition pas ignorée. Une association sûre (candidat unique dans les deux sens) est faite automatiquement après chaque import et à la demande ; les autres sont proposées dans l'écran Échéancier (Associer / Ignorer). L'échéancier montre les occurrences des 60 derniers jours avec leur statut (réalisée ou en attente) ; on peut associer à la main (même compte et même sens, à 15 jours près, montant libre) ou dissocier. Modifier une échéance oublie les associations qui ne tombent plus sur une de ses occurrences.
- **Détection automatique des abonnements** : repérage des opérations récurrentes non déclarées dans l'échéancier *(décisions du 21/10/2026)*. Les opérations d'un tiers sur un compte, dans un sens, forment une série quand chacune arrive une période après la précédente (hebdomadaire 6–8 jours, mensuelle 26–36, trimestrielle 85–98, semestrielle 175–190, annuelle 355–375) avec un montant à ± 10 % du dernier : 3 opérations au moins, 2 pour une série annuelle. Toutes les séries sont détectées (dépenses, revenus, virements internes). Une série terminée (plus d'une période et 10 jours sans opération), déjà associée à une échéance, ou couverte par une échéance du même compte, de même périodicité et de montant proche, n'est pas proposée. Les suggestions apparaissent dans l'échéancier : « Ajouter » pré-remplit le formulaire (tiers, jour médian, dernier montant, catégorie, début à la première opération) puis associe les opérations passées ; « Ignorer » est définitif pour ce tiers, ce compte et ce sens.
- **Alertes** : solde projeté négatif, dépense inhabituelle pour une catégorie, échéance manquante *(décisions du 22/10/2026)*. Calculées à la demande, affichées dans une carte « Alertes » en haut du tableau de bord (absente s'il n'y en a pas) et comptées par une pastille sur « Tableau de bord » dans la barre latérale. **Solde négatif** : un compte courant ou d'épargne dont le solde projeté passe sous zéro dans les 30 jours (les cartes à débit différé, négatives par nature, sont exclues) ; l'alerte disparaît seulement avec la prévision. **Dépense inhabituelle** : les dépenses du mois en cours d'une catégorie principale dépassent de plus de 50 % et d'au moins 50 € leur moyenne des 6 mois complets précédents, la catégorie ayant été dépensée au moins 3 de ces mois. **Échéance manquante** : une occurrence des 60 derniers jours non réalisée alors que le compte a une opération datée de plus de 5 jours après elle (l'import couvre la période) et qu'aucune association n'est proposée. Une dépense inhabituelle (catégorie et mois) et une échéance manquante (échéance et date) peuvent être masquées.
- **Prévision de trésorerie** : solde projeté à partir du solde actuel et de l'échéancier *(décisions du 22/10/2026)*. Le solde de fin de journée d'aujourd'hui (comptes courants, épargne et cartes non archivés, ou un seul compte) reçoit, jour par jour, les occurrences pas encore réalisées jusqu'à l'horizon (30, 60 ou 90 jours) ; une occurrence en retard de moins de 5 jours (la fenêtre de rapprochement) est comptée aujourd'hui, une plus ancienne est considérée comme manquée. Quand la vue couvre plusieurs comptes, les échéances de catégorie Virement interne sont ignorées : l'argent reste dans le total. La carte de l'échéancier montre la courbe, le solde à l'horizon, le point bas (signalé s'il est négatif) et les échéances prises en compte ; le tableau de bord affiche le solde projeté à 30 jours et son point bas.

#### Placements
- **Portefeuille détaillé** : titres, quantités, prix de revient, mouvements (achat, vente, dividende) *(décisions du 22/10/2026)*. Saisie à la main dans un écran Portefeuille par compte de placement (ouvert depuis l'écran Comptes) : positions (quantité, PRU, prix de revient, dividendes), mouvements et titres. Les titres (nom, ISIN facultatif vérifié par sa clé) sont communs à tous les comptes. Quantités à 6 décimales (fractions d'actions), stockées en millionièmes d'unité. Prix de revient au coût moyen pondéré, frais d'achat inclus ; une vente retire la part du prix de revient des parts vendues sans changer le PRU, et ne peut pas dépasser la quantité détenue à sa date (vérifié à chaque saisie, modification ou suppression). La valeur du compte et le patrimoine net restent ceux des valorisations jusqu'à V2-13.
- **Cours de bourse récupérés automatiquement** *(décisions du 22/10/2026)* : seule exception au « tout local », désactivée par défaut et activée dans les Paramètres. Source : Yahoo Finance (point d'accès `chart`, non officiel et sans clé ; saisie manuelle toujours possible). Seuls les symboles des titres (et l'ISIN lors d'une recherche de symbole) sont envoyés à query1.finance.yahoo.com. Une fois activée, mise à jour au démarrage au plus une fois par jour et à la demande (Paramètres, Portefeuille) : un an de cours de clôture pour un titre sans cours, un mois ensuite. Un cours saisi à la main n'est jamais remplacé ; seuls les cours en euros sont gardés. Le symbole se saisit dans la fiche du titre, ou se trouve à partir de l'ISIN. Le portefeuille affiche le dernier cours de chaque position.
- **Performance et plus-values**, latentes et réalisées.

#### Interface
- **Mode clair et mode sombre** : choix entre « clair », « sombre » et « système » (suit le réglage du système d'exploitation, par défaut), mémorisé dans les préférences. Le thème sombre redéfinit les variables CSS de `src/styles/tokens.css`, y compris les couleurs des catégories (pastilles et graphiques ECharts), pour garder un contraste lisible *(décisions du 22/10/2026)* : palette « violet nuit » (fond `#15111D`, cartes `#1F1A2B`, bouton principal violet clair `#8F7BD6`). Le choix est mémorisé dans le stockage local du webview, lu avant le premier affichage ; il ne fait pas partie du fichier de données.

#### Technique
- **Multi-devises** avec conversion vers la devise de référence.
- **Chiffrement** du fichier de données, protégé par mot de passe.
- **Sauvegarde complète** : sauvegardes planifiées, rotation, restauration depuis l'interface.

### Plus tard

- Import OFX / QIF
- Synchronisation bancaire automatique (agrégateur DSP2)
- Catégorisation par embeddings ou LLM local
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
- **Revenu ou dépense** : une opération est un revenu si sa catégorie est une catégorie de revenus, une dépense sinon, en net : un remboursement classé en Courses diminue les dépenses de Courses. Une opération non catégorisée est classée selon son signe.
- **Le patrimoine net** à une date D se calcule ainsi :
  - Σ soldes des comptes courants, d'épargne et des cartes à débit différé à D (l'encours d'une carte, négatif, est déjà de l'argent dépensé)
  - \+ dernière valorisation connue ≤ D de chaque compte de placement et de chaque actif
  - − dernier capital restant dû connu ≤ D de chaque dette
- **Le disponible** (tableau de bord) est la somme des soldes des comptes courants, d'épargne et des cartes à débit différé : l'encours des cartes en est déduit.
- **Par défaut, les comptes archivés** sont exclus des indicateurs. Un compte est pris en compte à partir de sa date d'ouverture, un compte valorisé à partir de sa première valorisation.
- **Les montants** sont stockés en **centimes (entiers)**, jamais en virgule flottante.
- **Le signe** est négatif pour un débit et positif pour un crédit, du point de vue du compte.

## Règles de gestion des opérations

- **Comptes concernés** : seuls les comptes courants, d'épargne et les cartes à débit différé ont des opérations. Les placements, actifs et dettes sont suivis par leurs valorisations.
- **Date** : une opération ne peut pas être antérieure à la date d'ouverture de son compte, puisqu'elle serait déjà comprise dans le solde d'ouverture.
- **Opération importée** : sa date, son montant et son libellé brut viennent de la banque et ne sont pas modifiables (détection des doublons, contrôle des soldes). Seules sa catégorie, sa date de valeur, sa note et ses tags le sont. Elle ne se supprime pas seule : on annule le lot d'import entier.
- **Opération saisie à la main** : entièrement modifiable et supprimable. Le texte saisi devient son libellé brut.

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
  is_internal_transfer -- derived from the category (linking a transfer sets the internal transfer category)
  transfer_pair_id     -- V2 other side of an internal transfer, both ways
  note, created_at, updated_at

operation_split          -- one row per operation in V1; several rows once splitting is supported
  id, operation_id, category_id, amount_cents

category
  id, name, parent_id, color, is_internal_transfer (system flag), is_income

import_batch
  id, account_id, source_type (CSV | PDF), source_file_name, source_file_hash,
  imported_at, statement_start_balance_cents, statement_end_balance_cents,
  balance_check_status (OK | MISMATCH | NOT_AVAILABLE)

csv_mapping
  id, name, delimiter, encoding, date_format, decimal_separator,
  column_date, column_label, column_amount, column_debit, column_credit, skip_lines,
  column_fee,                 -- optional, subtracted from the amount
  column_state, state_keep    -- optional, lines in another state are skipped

label_category_memory    -- V1 automatic category reuse
  normalized_label, category_id, last_used_at

categorization_rule      -- "normalized label contains X" → category, first match wins
  id, pattern, category_id, position

valuation                -- history of value for INVESTMENT / ASSET / LIABILITY accounts
  id, account_id, date, value_cents

scheduled_operation      -- schedule (display only in V1)
  id, account_id, label, amount_cents, category_id,
  frequency (WEEKLY | MONTHLY | QUARTERLY | SEMIANNUAL | YEARLY), day_of_period, start_date, end_date

schedule_match           -- V2 operation realizing an occurrence (occurrences are computed, keyed by date)
  scheduled_operation_id, occurrence_date, operation_id (unique)

schedule_match_dismissal -- V2 proposed matches the user dismissed
  scheduled_operation_id, occurrence_date, operation_id

recurring_dismissal      -- V2 recurring series the user does not want in the schedule
  account_id, payee_id, is_credit

unusual_expense_dismissal    -- V2 dismissed unusual expense alerts (month = first day)
  category_id, month

missing_occurrence_dismissal -- V2 dismissed missing occurrence alerts
  scheduled_operation_id, occurrence_date

tag                      -- V2 free labels, created on first use, unique ignoring case
  id, name

operation_tag
  operation_id, tag_id

transfer_dismissal       -- V2 proposed transfer pairs the user dismissed
  debit_id, credit_id

attachment               -- V2 files copied into attachments/, deleted with their operation
  id, operation_id, file_name (original), stored_name, size_bytes, added_at

security                 -- V2 securities, shared by the investment accounts
  id, name, isin (unique, nullable), symbol (Yahoo Finance, nullable), currency

investment_transaction   -- V2 buys, sells and dividends of an investment account
  id, account_id, security_id, date, kind (BUY | SELL | DIVIDEND),
  quantity_micro (millionths of a unit, 0 for a dividend), amount_cents (> 0), fees_cents, note

security_price           -- V2 daily price of a security; MANUAL prices are never overwritten
  security_id, date, price_cents, source (MANUAL | YAHOO)

app_setting              -- V2 settings kept with the data (price_updates_enabled, prices_updated_on)
  key, value
```

Note : la catégorie est portée par `operation_split` dès la V1. Ainsi, l'arrivée de la ventilation ne change aucune requête de statistique.

Conventions de stockage :
- **Dates** en `TEXT` ISO : `YYYY-MM-DD` pour les dates, `YYYY-MM-DDTHH:MM:SSZ` (UTC) pour les horodatages (`created_at`, `imported_at`…).
- **Énumérations** en `TEXT` avec une contrainte `CHECK` sur les valeurs autorisées.
- **Couleur de catégorie** : une famille de couleurs du design (`housing`, `groceries`, `restaurants`, `transport`, `leisure`, `subscriptions`, `health`, `neutral`), pas un code hexadécimal.
- **Migrations** : jusqu'à la première release, la migration initiale est modifiée directement (supprimer son `money.sqlite` local quand elle change) ; ensuite, chaque évolution du schéma est une nouvelle migration.

---

## Flux d'import

```text
File (CSV / PDF)
  → Parser (CSV mapping or bank-specific PDF parser)
  → Raw lines (date, label, amount) + statement balances when available
  → Balance check (start + Σ = end ?)          → warning if mismatch
  → Label normalization (regex)
  → Duplicate detection                         → ambiguous cases shown to the user
  → Categorization rules, then automatic category reuse (label memory)
  → Preview screen (user validates / corrects)
  → Automatic backup of the data file
  → Insert as a new import_batch
  → Internal transfer pairing (certain pairs linked, the others proposed)
  → Schedule matching (certain matches made, the others proposed)
```

Un import peut être annulé à tout moment en supprimant son lot.

---

## Ordre de développement V1

1. **Socle** : modèle de données, gestion des comptes, saisie manuelle des opérations
2. **Import CSV** : mapping des colonnes, détection des doublons, lots d'import et annulation, sauvegarde avant import
3. **Catégorisation** : catégories, virement interne, normalisation des libellés, reprise automatique de catégorie, propagation, règles de catégorisation
4. **Premiers graphiques** : dépenses par catégorie, évolution du solde, revenus vs dépenses
5. **Import PDF** : parsers Fortuneo et Trade Republic, liste des banques prises en charge, contrôle des soldes
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
| Base de données | **SQLite** via **sqlx** | Requêtes vérifiées à la compilation ; un seul fichier de données. Le cache `.sqlx/` (généré par `cargo sqlx prepare`, outil **sqlx-cli**) est commité pour compiler sans base, en CI notamment |
| Migrations | **sqlx migrate** | Fichiers SQL versionnés, appliqués au démarrage |
| Montants | `i64` en centimes | Jamais de flottant ; `rust_decimal` si des calculs de taux l'exigent |
| Dates | **chrono** (ou **time**) | |
| Erreurs | **thiserror** (domaine) / **anyhow** (outillage) | Erreurs typées remontées au front |
| Import CSV | crate **csv** + **encoding_rs** | Séparateur `;`, encodage ISO-8859-1 / Windows-1252 courants |
| Empreinte des fichiers importés | crate **sha2** | SHA-256 du fichier source (`source_file_hash`), pour signaler un fichier déjà importé |
| Import PDF | **pdfium-render** | Texte avec positions via PDFium ; bibliothèque PDFium embarquée dans le bundle. Version épinglée (binaires `bblanchon/pdfium-binaries`, `chromium/7881`, vérifiés par SHA-256), téléchargée dans `src-tauri/pdfium/` par `tools/fetch_pdfium.sh`, à lancer une fois avant de compiler |
| Normalisation des libellés | crate **regex** | |
| Cours de bourse (V2) | **reqwest** (rustls, json, query) | Appels HTTP côté Rust vers Yahoo Finance, TLS en pur Rust |
| Suggestion de catégorie (V2) | **linfa** (`linfa`, `linfa-logistic`, `ndarray`) | TF-IDF (calculé dans le module) + régression logistique multiclasse, entraînée en local sur les opérations catégorisées |
| Frontend | **Vue 3 + TypeScript + Vite** | |
| Navigation | **Vue Router** | Un écran par entrée de la barre latérale |
| Police | **Plus Jakarta Sans** | Fichiers `.woff2` (licence OFL) commités dans `src/assets/fonts/`, aucun chargement réseau |
| Graphiques | **Apache ECharts** (via **vue-echarts**) | Camemberts, séries temporelles, zoom sur une période |
| Typage front ↔ back | **tauri-specta** | Génère les types et fonctions TypeScript à partir des commandes Rust ; les `i64` (centimes) sont exportés en `number`, exact jusqu'à 2⁵³ |
| Accès système | **tauri-plugin-dialog**, **tauri-plugin-fs**, **tauri-plugin-opener** | Sélection des fichiers de relevés et des pièces jointes ; ouverture d'une pièce jointe avec l'application par défaut (appelé depuis Rust, le front ne peut ouvrir que des pièces jointes connues) |
| Tests | `cargo test` (back), **Vitest** (front) | Relevés réels anonymisés comme fixtures d'import |
| Packaging | Bundler Tauri | `.msi` / NSIS (Windows), `.deb`, `.rpm`, AppImage (Linux) |
| CI | **GitHub Actions** + **tauri-action** | Matrice `windows-latest` / `ubuntu-latest` |

#### Pistes pour l'IA (versions ultérieures)

- **Catégorisation** : le classifieur local (TF-IDF + régression logistique avec **linfa**) est prévu en V2. Plus tard, éventuellement : embeddings (**fastembed-rs** / **ort**, c'est-à-dire ONNX Runtime) + plus proches voisins sur les opérations déjà catégorisées.
- **Assistant** : LLM local via **Ollama** (crate **ollama-rs**). Le modèle appelle les fonctions de statistiques existantes (*tool calling*) au lieu de générer du SQL.
- L'IA reste **optionnelle** : elle est activée seulement si Ollama est détecté sur la machine.

#### Points d'attention

- **Moteur web différent selon l'OS** : WebView2 (Chromium) sous Windows, **WebKitGTK** sous Linux. Il faut tester le rendu et les performances d'ECharts sur les deux plateformes dès le début.
- **Dépendances Linux** : WebKitGTK est déclaré en dépendance du `.deb` / `.rpm` et embarqué dans l'AppImage.
- **Permissions Tauri 2** : chaque commande et chaque accès fichier doit être autorisé explicitement dans `src-tauri/capabilities/`.
- **Emplacement des données** : `app_data_dir()` de Tauri, c'est-à-dire `%APPDATA%\<app>` sous Windows et `~/.local/share/<app>` sous Linux. Les sauvegardes automatiques sont rangées dans un sous-dossier `backups/`, les pièces jointes dans `attachments/`.
- **Fichier de données** : `money.sqlite` (identifiant d'application `fr.cyprien.money`). SQLite reste dans son mode de journal par défaut, pas en WAL : le fichier est ainsi autonome et une simple copie suffit pour la sauvegarde avant import.
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
│       ├── labeling/             # label normalization, category memory, rules
│       └── stats/                # aggregation queries for charts (future AI tools)
├── tests/fixtures/               # anonymized real statements + expected results
├── tools/                        # dev scripts (anonymize_pdf.py: PDF anonymization; fetch_pdfium.sh: PDFium library)
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

- Profondeur de l'historique à importer au départ
