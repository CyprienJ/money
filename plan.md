# Plan de développement

Découpage en tickets de la V1 et de la V2 décrites dans `README.md`. Le README reste la spécification de référence : en cas de divergence, c'est lui qui fait foi.

- Les tickets V1 suivent l'**ordre de développement V1** du README (étapes 1 à 7), précédées d'une étape 0 d'amorçage.
- Chaque ticket est terminé quand ses critères d'acceptation sont remplis et que `cargo check`, `cargo test`, `cargo clippy` (dans `src-tauri/`) et `npm run build` passent.
- Un ticket ≈ un ou quelques commits cohérents.
- Les maquettes de référence sont dans `docs/design/` (Dashboard, Operations, Import, Analyses).
- 🟡 **Décision requise** : point non couvert par le README, à trancher avant d'implémenter le ticket.

Légende des tailles : **S** (≤ ½ jour), **M** (1–2 jours), **L** (3 jours et plus).

---

## V1 — Importer, consulter, visualiser

### Étape 0 — Amorçage du projet

#### V1-00.1 · Squelette Tauri 2 + Vue 3 — M
Remplacer le projet Cargo actuel (`Cargo.toml` et `src/main.rs` à la racine) par la structure du README : `src/` pour le front Vue 3 + TypeScript + Vite, `src-tauri/` pour le backend Rust.
- [x] `npm run tauri dev` ouvre une fenêtre vide sous Linux (WebKitGTK).
- [x] Arborescence Rust créée : `commands/`, `db/`, `account/`, `operation/`, `importing/{csv,pdf}/`, `labeling/`, `stats/`.
- [x] `.gitignore` couvre `target/`, `node_modules/`, `dist/`, `.idea/`.

#### V1-00.2 · Base SQLite et migrations — S
- [x] Pool `sqlx` (SQLite) ouvert sur un fichier dans `app_data_dir()`.
- [x] `sqlx migrate` appliqué au démarrage, dossier `src-tauri/migrations/`.
- [x] Type d'erreur de domaine (`thiserror`) sérialisable vers le front.

#### V1-00.3 · Pont tauri-specta — S
- [x] Une commande de démonstration (`app_version`) exposée et typée.
- [x] `src/bindings.ts` généré automatiquement en mode dev ; aucun appel `invoke` en dehors.
- [x] Capabilities Tauri 2 minimales dans `src-tauri/capabilities/`.

#### V1-00.4 · Design system et coque applicative — M
- [x] `src/styles/tokens.css` : couleurs, rayons, police, couleurs de catégories (tables de `CLAUDE.md`).
- [x] Plus Jakarta Sans (400–800) embarquée en `.woff2`, chiffres tabulaires.
- [x] Layout commun aux maquettes : barre latérale `#2A1E3F` (Tableau de bord, Opérations, Importer un relevé, Analyses ; section Patrimoine : Comptes, Échéancier, Catégories, Paramètres) + zone de contenu.
- [x] Vue Router avec une vue vide par entrée de menu.
- [x] Utilitaires front : formatage des montants en centimes → `1 234,56 €` (signe `−` typographique), des dates `fr-FR`.
- [x] Composant `CategoryPill` (pastille fond clair / texte foncé, variante pointillée « À catégoriser »).

#### V1-00.5 · Outillage qualité et CI — S
- [x] Vitest configuré côté front (fait avec V1-00.4).
- [x] GitHub Actions : `cargo check/test/clippy` + `npm run build` sur `ubuntu-latest` et `windows-latest`.
- [x] Workflow `tauri-action` pour les artefacts (`.msi`/NSIS, `.deb`, `.rpm`, AppImage).
- Vérification du rendu d'ECharts sous WebView2 et WebKitGTK : reportée à V1-04.2, à l'installation de vue-echarts.

---

### Étape 1 — Socle : modèle de données, comptes, saisie manuelle

#### V1-01.1 · Migration initiale du modèle de données — M
Créer toutes les tables V1 du README, y compris les colonnes *(prévu)* : `account`, `operation`, `operation_split`, `category`, `import_batch`, `csv_mapping`, `label_category_memory`, `valuation`, `scheduled_operation`.
- [x] Montants en `INTEGER` (centimes), clés étrangères et index (compte + date, `normalized_label`).
- [x] `currency` par défaut `'EUR'` ; `payee_id`, `transfer_pair_id`, `note` créés mais inutilisés.
- [x] Suppression en cascade : `import_batch` → `operation` → `operation_split`.

#### V1-01.2 · Module `account` (backend) — S
- [x] CRUD des comptes (types `CHECKING | SAVINGS | CREDIT_CARD | INVESTMENT | ASSET | LIABILITY`), archivage.
- [x] Calcul du solde d'un compte à une date : `opening_balance_cents + Σ amount_cents`.
- [x] Tests unitaires du calcul de solde.

#### V1-01.3 · Écran Comptes — M
- [x] Liste des comptes par type avec solde (pastille d'initiales comme dans le Dashboard).
- [x] Création / édition / archivage d'un compte (nom, type, établissement, solde et date d'ouverture).
- Pas de maquette : présentation alignée sur la carte « Comptes » du Dashboard (validée le 20/10/2026).

#### V1-01.4 · Module `operation` (backend) — M
- [x] Création / édition / suppression d'une opération manuelle, avec **une ligne `operation_split`** systématique.
- [x] `raw_label` saisi = libellé brut ; `normalized_label` calculé (stub jusqu'à V1-03.3).
- [x] Requête de liste par compte et période (sans pagination : le filtre par période suffit), avec solde courant par ligne.

#### V1-01.5 · Écran Opérations — L
Maquette : `Operations.dc.html`.
- [x] Sélecteur de compte (onglets), en-tête « solde au JJ/MM ».
- [x] Tableau : date, libellé normalisé + libellé brut en secondaire, pastille de catégorie, montant, solde.
- [x] Filtres : recherche texte, période, catégorie (dont « À catégoriser »).
- [x] Pied de tableau : nombre d'opérations, entrées, sorties, virements internes, opérations à catégoriser.
- [x] Modale « Nouvelle opération » et édition d'une opération.

---

### Étape 2 — Import CSV

#### V1-02.1 · Parser CSV et mapping des colonnes — M
- [x] Crates `csv` + `encoding_rs` : séparateur configurable, encodages UTF-8 / ISO-8859-1 / Windows-1252.
- [x] Mapping : format de date, séparateur décimal, colonne montant unique **ou** débit/crédit, lignes à ignorer.
- [x] Sortie commune `RawLine { date, value_date?, label, amount_cents }` (réutilisée par le PDF). Le mapping CSV n'a pas de colonne date de valeur : `value_date` reste vide pour un CSV.
- [x] Parsing des montants en centimes sans flottant ; tests sur fixtures anonymisées dans `tests/fixtures/`.
- Fixtures inventées au format bancaire français courant, faute de relevé réel : à valider sur les exports de la banque principale.

#### V1-02.2 · Mémorisation des mappings — S
- [x] CRUD `csv_mapping`, nommé par banque ou format.
- [x] Proposition automatique : chaque mapping enregistré est essayé sur le fichier ; ceux qui le lisent sans erreur sont proposés, le plus de lignes lues en premier (décision du 20/10/2026, sans lien compte ↔ mapping).

#### V1-02.3 · Détection des doublons — M
- [x] Doublon certain : même compte, même date, même montant, même libellé brut → exclu. Correspondances décomptées une à une (une opération existante ne couvre qu'une ligne).
- [x] Cas ambigus → présentés à l'utilisateur, non cochés par défaut (décision du 20/10/2026) :
  - ligne présente dans le compte, mais moins de fois que dans le fichier ;
  - même date et même montant qu'une opération existante, libellé différent ;
  - même montant qu'une opération **saisie à la main**, à ±3 jours.
- [x] Des lignes identiques dans un même fichier, absentes du compte, sont des opérations distinctes (nouvelles).
- [x] Tests unitaires couvrant les cas limites.

#### V1-02.4 · Lots d'import, sauvegarde et annulation — M
- [x] Hash du fichier source (`source_file_hash`, SHA-256 via `sha2`) ; `find_by_hash` pour l'alerte « fichier déjà importé » (affichée en V1-02.5).
- [x] Copie horodatée du fichier SQLite dans `app_data_dir()/backups/` avant chaque insertion (`VACUUM INTO`).
- [x] Insertion transactionnelle d'un `import_batch` et de ses opérations (non catégorisées).
- [x] Commande « annuler un import » : suppression du lot entier ; liste des lots d'un compte.
- Lignes antérieures à la date d'ouverture du compte : exclues et signalées dans l'aperçu (V1-02.5) ; `import` les refuse (décision du 20/10/2026).

#### V1-02.5 · Écran Import (assistant) — L
Maquette : `Import.dc.html` (étape 2 « Vérification et aperçu »).
- [x] Étape 1 « Fichier » : sélection via `tauri-plugin-dialog`, choix du compte de destination, choix / création du mapping CSV (appelé « format » dans l'interface), avec le début du fichier affiché en colonnes et le format reconnu présélectionné.
- [x] Étape 2 : compteurs (lues, doublons ignorés, nouvelles, à vérifier, antérieures à l'ouverture, à catégoriser), tableau des nouvelles opérations avec case « Inclure » (lignes ambiguës décochées, avec le motif), blocs repliables des doublons ignorés et des lignes antérieures à l'ouverture, alerte si le fichier a déjà été importé, mention de la sauvegarde. Carte « Soldes non vérifiés » pour un CSV. Le compteur « catégorisées automatiquement » est ajouté par V1-03.4.
- [x] Retirer les `allow(dead_code)` temporaires de `importing::batch` et `importing::duplicates`.
- [x] Étape 3 : import, puis retour à l'écran Opérations.
- [x] Bandeau « Dernier import … · Annuler cet import » sur l'écran Opérations, avec confirmation.
- Étape 1 et édition du mapping sans maquette : style de `Import.dc.html` (validé le 20/10/2026).

---

### Étape 3 — Catégorisation

#### V1-03.1 · Catégories (backend) — M
- [x] Lecture seule `list_categories` (avancée au ticket V1-01.5 pour l'écran Opérations).
- [x] CRUD sur deux niveaux (`parent_id`), indicateur `is_income` ; couleur et `is_income` hérités de la catégorie principale ; suppression refusée s'il reste des sous-catégories, avec réaffectation des opérations.
- [x] Catégorie système « Virement interne » (`is_internal_transfer`), non modifiable et non supprimable.
- [x] Arborescence par défaut créée au premier lancement (migration initiale), avec les couleurs du design (décision du 20/10/2026, inscrite au README).
- [x] `operation.is_internal_transfer` dérivé de la catégorie à chaque modification, y compris lors de la réaffectation d'une catégorie supprimée.

#### V1-03.2 · Écran Catégories — M
- [x] Liste arborescente (une carte par catégorie principale, avec le nombre d'opérations), création, renommage, déplacement, changement de couleur, suppression (avec réaffectation des opérations).
- Pas de maquette : présentation alignée sur les cartes de l'écran Comptes.

#### V1-03.3 · Normalisation des libellés — M
Module `labeling/`.
- [x] Regex : préfixes bancaires (`CB`, `PRLV SEPA`, `VIR`, `VIR SEPA`…), dates, numéros de carte, IBAN, références d'échéance et codes de référence.
- [x] Ex. `CB CARREFOUR MKT 18/10 CARTE 4417` → `CARREFOUR MKT`.
- [x] `raw_label` jamais modifié ; tous les `normalized_label` sont recalculés au démarrage (seules les lignes modifiées sont réécrites).
- [x] Tests unitaires sur un jeu de libellés réels anonymisés (relevés Fortuneo et Trade Republic, maquettes, formats courants).
- [x] Affichage : capitale initiale par mot, petits mots en minuscules (« Pharmacie du Centre »), calculé côté front (décision du 20/10/2026) ; les noms lisibles viendront avec les tiers en V2.

#### V1-03.4 · Reprise automatique de catégorie — S
- [x] Mise à jour de `label_category_memory` à chaque catégorisation manuelle (saisie, modification, changement de catégorie ; un retour à « à catégoriser » ne l'efface pas).
- [x] À l'import, catégorie proposée depuis la mémoire, avec indice « Reprise de « … » » ou « Libellé inconnu », enregistrée avec l'opération ; compteur « catégorisées automatiquement » dans l'aperçu.
- [x] Tests unitaires.

#### V1-03.5 · Catégorisation dans l'UI — S
- [x] Sélecteur de catégorie (pastille cliquable) dans l'aperçu d'import et dans l'écran Opérations ; une catégorie choisie dans l'aperçu, différente de la proposition, est retenue pour le libellé.
- [x] Mention « Exclu des statistiques » sur les virements internes ; montant en gris.

---

### Étape 4 — Premiers graphiques

#### V1-04.1 · Requêtes de statistiques — M
Module `stats/`, signatures claires (futurs outils de l'assistant IA). Toutes passent par `operation_split` et excluent les virements internes.
- [x] `expenses_by_category(from, to, accounts?, level: topLevel | detailed)` — dépenses nettes des remboursements.
- [x] `balance_history(from, to, accounts?, step: day | week | month)` — virements internes inclus, chaque compte à partir de sa date d'ouverture.
- [x] `income_vs_expenses(from, to, accounts?, granularity: month | year)` — périodes vides incluses.
- [x] Tests unitaires des agrégations (dont l'exclusion des virements internes).
- Revenu ou dépense selon la catégorie (`is_income`), non catégorisé selon le signe (décision du 20/10/2026, inscrite au README). Par défaut, comptes non archivés ayant des opérations.

#### V1-04.2 · Intégration ECharts — S
- [x] `vue-echarts` installé, thème ECharts basé sur les tokens CSS (composant `AppChart`, rendu SVG, imports réduits aux graphiques utilisés).
- [x] Rendu et performances d'ECharts vérifiés sous WebKitGTK (Linux) et Chromium, moteur de WebView2 : rendu identique ; 4 graphiques dont 3 650 points en ~120 ms (Chromium) et ~440 ms (WebKitGTK en rendu logiciel). Reste à confirmer dans la vraie fenêtre Windows.
- [x] Builders d'options dans `src/charts/` : revenus et dépenses, barres et anneau par catégorie, évolution du solde avec zoom.

#### V1-04.3 · Écran Analyses (première version) — L
Maquette : `Analyses.dc.html`.
- [x] Sélecteur de période Mois / Trimestre / Année, avec navigation ‹ › vers les périodes précédentes (ajout hors maquette).
- [x] « Revenus et dépenses » : barres groupées sur les 12 mois qui finissent avec la période + totaux revenus, dépenses, épargné (montant et %).
- [x] Dépenses par catégorie de la période (barres avec montants et anneau avec total).
- [x] Évolution du solde sur 12 mois, par compte et tous comptes confondus, avec zoom.
- Écran chargé à la demande (ECharts, ~620 kB, n'alourdit pas le démarrage). Comparaison avec l'année précédente, patrimoine net, top tiers et plus grosses dépenses de la maquette : étapes 6 et 7.

---

### Étape 5 — Import PDF

#### V1-05.1 · Infrastructure PDF — M
- [x] `pdfium-render` avec la bibliothèque PDFium embarquée dans le bundle (Linux et Windows) : `tools/fetch_pdfium.sh` (version épinglée, SHA-256), ressources `tauri.linux.conf.json` / `tauri.windows.conf.json`, téléchargement en CI. Windows non testé localement : à confirmer par la CI.
- [x] Extraction du texte avec positions (lignes triées, éléments de gauche à droite), testée sur les relevés Fortuneo et Trade Republic.
- [x] Trait `StatementParser` : détection du format, extraction des `RawLine` + soldes initial et final + période.
- [x] Détection automatique du parser à partir du contenu (commande `detect_pdf_statement`), liste `supported_banks`. Le « Format reconnu : … » s'affichera avec les parsers (V1-05.2).
- `allow(dead_code)` temporaire du module `importing::pdf` retiré en V1-05.2.

#### V1-05.2 · Parsers Fortuneo et Trade Republic — L
- [x] Un parser par banque : Fortuneo (compte courant), Trade Republic (compte courant, la partie PEA du relevé est ignorée) ; fixtures anonymisées dans `tests/fixtures/` + résultats attendus. Trade Republic : chaque ligne est vérifiée contre le solde imprimé après elle.
- [x] Liste des banques prises en charge, exposée au front et affichée à l'étape 1 de l'import ; « Format reconnu : … » à l'étape 1 et dans l'aperçu, avec les soldes du relevé.
- [x] Aperçu et import communs au CSV et au PDF (`preview_import`, `import_statement`), soldes du relevé enregistrés sur le lot.
- Les autres banques françaises s'ajoutent ensuite, un ticket par banque, dès qu'un relevé anonymisé est fourni (outil : `tools/anonymize_pdf.py`).

#### V1-05.3 · Contrôle des soldes — S
- [x] `start + Σ opérations = end` → `balance_check_status` `OK | MISMATCH | NOT_AVAILABLE`, sur toutes les lignes lues du relevé (doublons et lignes antérieures à l'ouverture compris) ; calculé dans l'aperçu (`balanceCheck`) et enregistré sur le lot.
- [x] Tests unitaires.

#### V1-05.4 · UI du contrôle des soldes — S
- [x] Bloc « Soldes vérifiés » de la maquette Import (solde initial, somme, solde calculé, verdict) ; bloc « Écart de solde » avec le solde final du relevé et le montant de l'écart.
- [x] En cas d'écart : import signalé comme suspect, choix annuler (Retour) / « Importer quand même ».
- [x] « soldes vérifiés » / « écart de solde » dans le bandeau du dernier import (écran Opérations), en rouge en cas d'écart avec la marche à suivre.
- « Corriger » = importer quand même, puis saisir à la main l'opération manquante (décision du 20/10/2026, inscrite au README).

---

### Étape 6 — Patrimoine

#### V1-06.1 · Valorisations — M
- [x] CRUD `valuation` pour les comptes `INVESTMENT`, `ASSET` et `LIABILITY` (valeur ou capital restant dû, à une date ; positive, une par date, pas avant l'ouverture du compte).
- [x] Historique des valorisations dans la fiche du compte ; solde des comptes valorisés = dernière valorisation à la date (négatif pour une dette) ; mentions « Valorisé au … », « estimé en … », « capital restant dû » dans l'écran Comptes.

#### V1-06.2 · Calcul du patrimoine net — M
- [x] `net_worth_at(date)` : Σ soldes courants, épargne et cartes à débit différé + dernière valorisation ≤ D (placements, actifs) − dernier capital restant dû ≤ D (dettes), avec le détail par famille.
- [x] `net_worth_history(from, to, step)`.
- [x] Tests unitaires.
- Cartes à débit différé : encours compté dans le patrimoine et déduit du « Disponible » (décision du 20/10/2026, inscrite au README) ; le libellé du Disponible sur le tableau de bord deviendra « Comptes courants et épargne, cartes déduites ».

#### V1-06.3 · Graphique du patrimoine net — S
- [x] Carte « Patrimoine net » de l'écran Analyses : valeur à la fin de la période, variation sur les 12 mois, courbe des fins de mois (ECharts, sans axes, comme la maquette).

---

### Étape 7 — Finitions

#### V1-07.1 · Échéancier (affichage seulement) — M
- [x] CRUD `scheduled_operation` (libellé, compte, montant, catégorie, périodicité, jour, début, fin).
- [x] Calcul des prochaines occurrences sur N jours, **sans créer d'opérations**.
- [x] Écran Échéancier : prochaines échéances sur 30 jours, liste des opérations récurrentes, fenêtre de saisie.
- Périodicités hebdomadaire, mensuelle, trimestrielle, semestrielle, annuelle ; jour = jour de la semaine (hebdomadaire) ou du mois, ramené au dernier jour des mois courts (décision du 20/10/2026, inscrite au README). Pas de maquette : style des cartes de l'écran Comptes.

#### V1-07.2 · Comparaison de périodes — M
- [x] Requête de comparaison (`compare_expenses_by_category`) : période vs précédente, ou vs même période l'an dernier.
- [x] Analyses : « Par catégorie » avec barre fine de la période comparée et badge de variation en % (rouge en hausse, vert en baisse).
- [x] Sélecteur « Comparer à … » : même période l'an dernier par défaut (comme la maquette), ou période précédente.

#### V1-07.3 · Top tiers et plus grosses dépenses — S
- [x] `top_payees(from, to, accounts?, limit)` et `largest_expenses(from, to, accounts?, limit)` sur les libellés normalisés, virements internes et revenus exclus.
- [x] Cartes correspondantes dans Analyses : « Top tiers sur 12 mois » et tuiles « Plus grosses dépenses » de la période.
- « Hors crédit » abandonné : le top porte sur toutes les dépenses, crédit compris, comme au tableau de bord (décision du 20/10/2026, inscrite au README).

#### V1-07.4 · Tableau de bord — L
Maquette : `Dashboard.dc.html`.
- [x] Navigation par mois (‹ Octobre 2026 ›), bouton « Importer un relevé ».
- [x] Indicateurs : patrimoine net (+ variation vs fin du mois précédent), disponible (courants + épargne, cartes déduites), dépenses du mois (vs mois précédent), revenus du mois (+ reste).
- [x] Dépenses par catégorie (anneau + légende, petites catégories regroupées), évolution du disponible sur 12 mois.
- [x] Carte Comptes (soldes, dernier import / dernière valorisation, « + Ajouter »).
- [x] Prochaines échéances (15 jours) et top tiers du mois (barres ECharts, comme les autres graphiques).
- État vide (aucun compte) : invitation à créer un compte ou importer un relevé. Écran chargé à la demande avec ECharts.

#### V1-07.5 · Paramètres et finitions — S
- [ ] Écran Paramètres : emplacement des données, dossier des sauvegardes, mappings CSV enregistrés.
- [ ] États vides (aucun compte, aucune opération) et messages d'erreur métier lisibles.
- [ ] Recette complète sur Windows et Linux, packaging de la V1.

---

## V2 — Automatiser et enrichir

Ordre indicatif : catégorisation et tiers d'abord (gain quotidien), puis opérations, suivi, placements, technique. À re-prioriser à la fin de la V1.

### Catégorisation et tiers

#### V2-01 · Règles de catégorisation — L
- [ ] Table `categorization_rule` : conditions « libellé contient X », « montant entre A et B », compte ; priorité.
- [ ] Application à l'import avant la mémoire de libellés ; écran de gestion et test d'une règle sur l'historique.

#### V2-02 · Gestion des tiers — L
- [ ] Tables `payee` et `payee_alias` ; exploitation de `operation.payee_id`.
- [ ] Fusion de plusieurs libellés normalisés en un tiers, renommage.
- [ ] Top tiers et analyses basés sur les tiers.

#### V2-03 · Appariement des virements internes — M
- [ ] Liaison automatique débit compte A ↔ crédit compte B (montant opposé, dates proches) via `transfer_pair_id`.
- [ ] `is_internal_transfer` explicite ; validation manuelle des appariements incertains.

### Opérations

#### V2-04 · Tags et notes — M
- [ ] Tables `tag` et `operation_tag` ; exploitation de `operation.note`.

#### V2-05 · Pièces jointes — M
- [ ] Table `attachment` ; fichiers copiés à côté du fichier de données ; ouverture depuis l'opération.

#### V2-06 · Recherche et filtres avancés — M
- [ ] Filtres combinés : montant, période, tiers, catégorie, tag, compte.

### Suivi

#### V2-07 · Rapprochement échéancier ↔ import — M
- [ ] Association d'une opération importée à une échéance (montant à une tolérance près, date à ± N jours) ; statut « réalisée ».

#### V2-08 · Détection automatique des abonnements — M
- [ ] Repérage des opérations récurrentes absentes de l'échéancier, proposition d'ajout.

#### V2-09 · Prévision de trésorerie — M
- [ ] Solde projeté à partir du solde actuel et de l'échéancier.

#### V2-10 · Alertes — M
- [ ] Solde projeté négatif, dépense inhabituelle pour une catégorie, échéance manquante.

### Placements

#### V2-11 · Portefeuille détaillé — L
- [ ] Tables `security` et `investment_transaction` : titres, quantités, prix de revient, achats, ventes, dividendes.

#### V2-12 · Cours de bourse automatiques — M
- [ ] Table `security_price` ; récupération des cours.
- 🟡 Seule exception au « tout local » : source des cours et consentement à valider.

#### V2-13 · Performance et plus-values — M
- [ ] Plus-values latentes et réalisées, performance ; patrimoine net basé sur la valorisation du portefeuille.

### Technique

#### V2-14 · Multi-devises — L
- [ ] Exploitation des colonnes `currency` ; conversion vers la devise de référence dans toutes les statistiques.

#### V2-15 · Chiffrement du fichier de données — M
- [ ] Passage à SQLCipher, mot de passe à l'ouverture, migration d'un fichier existant.

#### V2-16 · Sauvegarde complète — M
- [ ] Sauvegardes planifiées, rotation, restauration depuis l'interface.

---

## Décisions à prendre (récapitulatif)

| Ticket | Question |
|---|---|
| V1-07.5 | Design de l'écran Paramètres (sans maquette) |
| V2-12 | Source des cours de bourse (accès réseau) |
