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

#### V1-02.6 · Détection automatique du format CSV — M
Ajouté le 21/10/2026 (retour de recette sur un export Revolut).
- [x] Analyse du début du fichier : encodage, séparateur, ligne d'en-tête et lignes à ignorer, colonnes date / libellé / montant (titres puis contenu), format des dates, séparateur décimal, montant signé ou débit / crédit.
- [x] Proposition retenue seulement si elle lit tout le fichier sans erreur.
- [x] Bouton « Détecter automatiquement » dans la fenêtre du format, lancé d'office pour un nouveau format créé depuis un fichier.
- [x] Tests sur les fixtures CSV et un export de type Revolut.

#### V1-02.7 · Frais et état des opérations dans le format CSV — S
Ajouté le 21/10/2026 (export Revolut : colonnes « Fee » et « State »).
- [x] Colonne de frais facultative, déduite du montant (le compte perd montant + frais) ; frais vides comptés à zéro.
- [x] Colonne d'état facultative et valeur à garder (ex. `COMPLETED`, sans tenir compte de la casse) : les autres lignes (`PENDING`, `REVERTED`, `DECLINED`) sont ignorées avant lecture de la date et du montant.
- [x] Colonnes ajoutées à `csv_mapping` dans la migration initiale (règle du README maintenue le 21/10/2026) ; modèle de données du README mis à jour.
- [x] Détection automatique des colonnes « Fee » / « Frais » et « State » / « État » / « Statut » (valeur la plus fréquente gardée).
- [x] Champs « Colonnes facultatives » dans la fenêtre du format.
- 🟡 Valider sur quelques lignes anonymisées d'un vrai export Revolut (en-tête, ligne avec frais, ligne non `COMPLETED`).

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

#### V1-03.6 · Dates en toutes lettres et entités HTML dans la normalisation — S
Ajouté le 21/10/2026 (retour de recette : « Intérêts nets versés pour Compte d&#39;épargne le Apr 5, 2025 » donne un libellé différent chaque mois).
- [x] Décodage des entités HTML (`&#39;`, `&amp;`…).
- [x] Suppression des dates en toutes lettres, en anglais (« Apr 5, 2025 ») et en français (« 5 avr. 2025 », « 1er avril 2025 »), et d'un mois suivi de son année (« Loyer avril 2025 »). Un mois seul est gardé (« Salaire sept », « Mars Distribution »).
- [x] Opérations existantes renormalisées au démarrage (mécanisme existant) ; la catégorie retenue pour l'ancien libellé est reportée sur le nouveau. Tests unitaires.

#### V1-03.7 · Propagation de la catégorie — S
Ajouté le 21/10/2026.
- [x] Aperçu d'import : une catégorie choisie sur une ligne s'applique aux autres lignes du même libellé normalisé, sauf celles choisies à la main.
- [x] Écran Opérations : après une catégorisation, bandeau « Classer aussi les N autres « … » dans … ? » pour les opérations du même libellé encore à catégoriser ; avertissement si d'autres opérations du même libellé sont dans une autre catégorie.
- [x] Commandes fines : compte des opérations concernées, catégorisation groupée (une requête, mémoire mise à jour) ; tests unitaires.

#### V1-03.8 · Règles de catégorisation — M
Avancé de la V2 le 21/10/2026. Une seule condition : « libellé normalisé contient X », sans tenir compte des majuscules ni des accents.
- [x] Table `categorization_rule` (migration initiale), CRUD et réordonnancement ; la première règle qui correspond l'emporte.
- [x] À l'import, règles appliquées avant la reprise automatique, avec l'indice « Règle « … » ».
- [x] Application d'une règle aux opérations existantes encore à catégoriser, avec le nombre d'opérations concernées.
- [x] Écran de gestion des règles (section de l'écran Catégories, ordre modifiable, bouton « Classer ») ; création rapide depuis la fenêtre d'une opération. Une règle suit la catégorie qui reprend les opérations d'une catégorie supprimée, et disparaît sinon.
- [x] Tests unitaires.

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
- [x] Écran Paramètres : emplacement des données, dossier des sauvegardes (avec leur nombre), mappings CSV enregistrés, banques PDF prises en charge, version.
- [x] États vides (aucun compte, aucune opération) et messages d'erreur métier lisibles : erreurs structurées (domaine, code, paramètres) traduites en français côté front, avec vérification d'exhaustivité au typage.
- [x] Recette complète sur Windows et Linux, packaging de la V1.
  - Fait : paquet Linux `.deb` construit (`npm run tauri build -- --bundles deb`), PDFium et sa licence embarqués dans `/usr/lib/Finances perso/pdfium/` ; binaire release démarré sur une base vierge (migration et catégories par défaut OK).
  - Recette validée le 21/10/2026. Paquets Windows (`.msi` / NSIS) via le workflow `release.yml`.

---

## V2 — Automatiser et enrichir

Ordre validé le 21/10/2026, à la fin de la V1 :

1. V2-02 · Gestion des tiers (socle des filtres, des analyses et de la détection des abonnements)
2. V2-04, V2-06, V2-05 · Notes et tags, recherche et filtres avancés, pièces jointes
3. V2-03 · Appariement des virements internes
4. V2-07 à V2-10 · Suivi : rapprochement, prévision, abonnements, alertes
5. V2-01 · Classifieur local (il apprend mieux avec plus d'opérations catégorisées)
6. V2-17 · Mode clair et mode sombre (indépendant, peut être intercalé)
7. V2-11 à V2-13 · Placements détaillés
8. V2-16, V2-15, V2-14 · Technique : sauvegarde complète, chiffrement, multi-devises
9. V2-18 · Import OFX et QIF (ajouté le 22/10/2026, avancé depuis « Plus tard »)
10. V2-19 · Mouvements titres des relevés Trade Republic (ajouté le 22/10/2026)
11. V2-20 · Mémoire des relevés importés (ajouté le 22/10/2026)

Depuis la release 1.0.0, chaque évolution du schéma est une nouvelle migration.

### Catégorisation et tiers

#### V2-01 · Suggestion de catégorie par classifieur local — M
Les règles de catégorisation prévues initialement ici ont été avancées en V1 (V1-03.8, décision du 21/10/2026). Ticket remplacé le 21/10/2026 par le classifieur local, avancé depuis « Plus tard ».
Décisions du 22/10/2026 : dépendances `linfa`, `linfa-logistic` et `ndarray`, sans `linfa-preprocessing` (son TF-IDF calcule l'IDF sur les documents transformés, pas sur l'apprentissage) ; 30 libellés distincts catégorisés et 2 catégories au moins, confiance de 50 % au moins ; aperçu d'import avec ✓ par ligne et « Accepter les N suggestions » ; écran Opérations avec suggestion sous la catégorie et pré-sélection dans la fiche.
- [x] Dépendances `linfa`, `linfa-logistic` et `ndarray`.
- [x] Module `labeling/classifier` : TF-IDF sur le libellé normalisé (mots et trigrammes de caractères, IDF appris) plus le sens du montant, régression logistique multiclasse ; un exemple par libellé distinct et sens, avec sa catégorie la plus fréquente. Modèle en mémoire, entraîné au démarrage puis en tâche de fond quand les opérations catégorisées changent ; les suggestions n'attendent jamais l'entraînement.
- [x] Seuils : 30 libellés distincts et 2 catégories pour entraîner, 50 % de confiance pour suggérer.
- [x] Aperçu d'import : « Suggestion : Courses (72 %) » pour les lignes sans règle ni mémoire, ✓ par ligne et « Accepter les N suggestions » ; une suggestion acceptée est un choix, retenu pour le libellé.
- [x] Écran Opérations : suggestion sous la catégorie des opérations à catégoriser (✓ pour accepter), pré-sélectionnée dans la fiche.
- [x] Tests : apprentissage sur un jeu anonymisé, libellés inconnus proches de libellés connus, seuils, exemples tirés de la base.

#### V2-02 · Gestion des tiers — L
Décisions du 21/10/2026 : chaque libellé normalisé reçoit automatiquement un tiers, nommé par défaut avec sa forme lisible (« Carrefour Mkt ») ; les tiers se gèrent dans un nouvel écran « Tiers » ; les écrans affichent le nom du tiers à la place du libellé normalisé.
- [x] Tables `payee` et `payee_alias` ; exploitation de `operation.payee_id`.
- [x] Fusion de plusieurs libellés normalisés en un tiers, renommage, détachement d'un libellé.
- [x] Top tiers et analyses basés sur les tiers.

#### V2-03 · Appariement des virements internes — M
Décisions du 21/10/2026 : lier = catégorie « Virement interne » (l'exclusion reste dérivée de la catégorie) ; appariements sûrs liés automatiquement, incertains proposés ; ± 3 jours ; validation par un bandeau et une fenêtre dans l'écran Opérations.
- [x] Liaison automatique débit compte A ↔ crédit compte B (montant opposé, dates proches) via `transfer_pair_id`, après chaque import et à la demande sur l'historique.
- [x] Validation manuelle des appariements incertains (Lier / Ignorer, refus mémorisés dans `transfer_dismissal`).
- [x] Fiche d'une opération : pendant affiché, Délier, Lier à la main ; changer de catégorie délie.

### Import

#### V2-18 · Import OFX et QIF — L
Décisions du 22/10/2026 : catégories QIF associées par nom ou par un choix retenu, modifiables dans l'aperçu ; ventilations importées en une ligne, détail dans la note ; mémo dans la note ; un seul compte par fichier, placements refusés.
- [x] Lecteurs OFX 1.x / 2.x et QIF écrits à la main (module `importing/interchange`), sans dépendance ; migration 0012 (`source_type` OFX et QIF, table `source_category_mapping`).
- [x] Branchement dans l'aperçu et l'import : solde d'ouverture QIF, solde final OFX, devise OFX contrôlée, virements `[Compte]` en Virement interne, notes.
- [x] Écran Importer : format reconnu à l'extension, carte « Catégories du fichier ».
- [x] Tests : exports Money 2000 (Windows-1252, dates `31/12' 0`, ventilations), OFX SGML et XML, refus (plusieurs comptes, placements, dates et montants illisibles), import complet avec choix retenus.

#### V2-20 · Mémoire des relevés importés — M
Décisions du 22/10/2026 : une copie par contenu (empreinte SHA-256) ; PDF ouverts dans le lecteur du système (pas de rendu des pages, pas de nouvelle dépendance), fichiers texte dans une visionneuse intégrée ; mouvements titres liés aussi ; relevés dans les sauvegardes complètes, en clair.
- [x] Migration 0013 : `import_batch.stored_file`, `operation.source_page` / `source_line`, `investment_transaction.import_batch_id` / `source_page`.
- [x] Position de chaque ligne relevée par tous les lecteurs (page PDF, ligne CSV, OFX, QIF) ; module `importing/source_document` (copie, nettoyage, lecture du texte) ; import des mouvements titres en lot sans opération.
- [x] Fiches d'opération et de mouvement : « Relevé source », visionneuse texte avec la ligne surlignée.
- [x] Sauvegardes complètes : copie et restauration de `statements/`.

### Opérations

#### V2-04 · Tags et notes — M
Décisions du 21/10/2026 : note et tags modifiables sur toutes les opérations, importées comprises ; tags créés à la volée depuis la fiche d'une opération, gérés (renommage, suppression) dans une carte « Tags » de l'écran Catégories ; pas de couleur de tag ; affichage sous le libellé dans la liste des opérations.
- [x] Tables `tag` et `operation_tag` ; exploitation de `operation.note`.
- [x] Note et tags dans la fiche d'une opération, avec suggestion des tags existants.
- [x] Tags et note affichés dans la liste des opérations et couverts par la recherche libre (le filtre par tag reste pour V2-06).
- [x] Renommage et suppression des tags dans l'écran Catégories.

#### V2-05 · Pièces jointes — M
Décisions du 21/10/2026 : ouverture par **tauri-plugin-opener** (dépendance validée), appelé depuis Rust ; fichiers supprimés avec leur opération (et nettoyage des orphelins au démarrage) ; 25 Mo au plus par fichier.
- [x] Table `attachment` ; fichiers copiés à côté du fichier de données ; ouverture depuis l'opération.
- [x] Section « Pièces jointes » dans la fiche d'une opération : ajout (plusieurs fichiers à la fois), ouverture, suppression.
- [x] Nombre de pièces jointes affiché dans la liste des opérations.

#### V2-06 · Recherche et filtres avancés — M
Décisions du 21/10/2026 : recherche dans l'écran Opérations, avec un onglet « Tous les comptes » et un panneau « Plus de filtres » ; montant filtré par sens et fourchette en valeur absolue ; période personnalisée ; une seule valeur par filtre.
- [x] Filtres combinés : montant, période, tiers, catégorie, tag, compte (requête `operation::search`, commande `search_operations`).
- [x] Onglet « Tous les comptes » (comptes archivés exclus), colonne Compte à la place du solde.
- [x] Période personnalisée et panneau « Plus de filtres » : sens, montant min–max, tiers, tag ; réinitialisation des filtres.
- [x] La recherche libre reste côté front (sans accents) et s'applique au résultat des filtres.

### Suivi

#### V2-07 · Rapprochement échéancier ↔ import — M
Décisions du 21/10/2026 : montant à ± 10 %, date à ± 5 jours ; associations sûres automatiques (après import et à la demande), les autres proposées ; occurrences des 60 derniers jours affichées avec leur statut.
- [x] Association d'une opération importée à une échéance (montant à une tolérance près, date à ± N jours) ; statut « réalisée ».
- [x] Propositions à valider (Associer / Ignorer), association et dissociation à la main depuis l'échéancier.
- [x] Échéances récentes (60 jours) avec statut, échéances à venir déjà réalisées marquées (échéancier et tableau de bord).

#### V2-08 · Détection automatique des abonnements — M
Décisions du 21/10/2026 : 3 opérations au moins (2 pour l'annuel) ; toutes les séries (dépenses, revenus, virements internes) ; suggestions dans une carte de l'échéancier ; ajout suivi de l'association des opérations passées.
- [x] Repérage des opérations récurrentes absentes de l'échéancier (module `operation/schedule/detection`), proposition d'ajout.
- [x] « Ajouter » pré-remplit le formulaire d'échéance puis associe les opérations passées ; « Ignorer » mémorisé dans `recurring_dismissal`.

#### V2-09 · Prévision de trésorerie — M
Décisions du 22/10/2026 : carte dans l'échéancier et KPI sur le tableau de bord ; horizon 30, 60 ou 90 jours ; occurrence en retard de moins de 5 jours comptée aujourd'hui ; échéances de virement interne ignorées quand la vue couvre plusieurs comptes.
- [x] Solde projeté jour par jour à partir du solde actuel et des occurrences non réalisées (`stats::balance_forecast`), avec le point bas.
- [x] Carte « Prévision de trésorerie » (compte, horizon, courbe, échéances prises en compte) et KPI « Solde projeté à 30 j ».

#### V2-10 · Alertes — M
Décisions du 22/10/2026 : solde négatif par compte courant ou d'épargne à 30 jours ; dépense inhabituelle = mois en cours > moyenne des 6 mois précédents + 50 % et + 50 € (3 mois de dépense au moins) ; échéance manquante quand l'import couvre la date + 5 jours ; dépenses inhabituelles et échéances manquantes masquables ; carte du tableau de bord et pastille dans la barre latérale.
- [x] Module `alert` : trois règles calculées à la demande, masquage dans `unusual_expense_dismissal` et `missing_occurrence_dismissal`.
- [x] Carte « Alertes » du tableau de bord (lien vers l'écran concerné, « Masquer ») et pastille sur « Tableau de bord ».

### Placements

#### V2-11 · Portefeuille détaillé — L
Décisions du 22/10/2026 : quantités à 6 décimales ; prix de revient au coût moyen pondéré, frais d'achat inclus ; saisie à la main (import des ordres Trade Republic éventuellement dans un ticket à part) ; valeur du compte et patrimoine net inchangés jusqu'à V2-13 ; écran Portefeuille par compte de placement.
- [x] Tables `security` et `investment_transaction` : titres, quantités, prix de revient, achats, ventes, dividendes (module `investment`).
- [x] Écran Portefeuille (`/accounts/:id/portfolio`) : positions, mouvements, titres ; lien depuis l'écran Comptes.

#### V2-19 · Mouvements titres des relevés Trade Republic — M
Décisions du 22/10/2026 : import du PDF dans le compte de placement (un import par compte) ; montant total frais compris, frais à 0 ; titres inconnus créés automatiquement (ISIN et nom du relevé).
- [x] Lecteur : section titres du relevé (soldes vérifiés), lignes « Buy trade », « Sell trade », « Savings plan execution » (cette dernière sans exemple réel) ; arrêt du tableau au pied de page.
- [x] Module `importing/trade_import` : aperçu (doublons par ISIN, date, sens, quantité et montant + frais ; titres à créer ; antérieurs à l'ouverture), import d'un bloc après sauvegarde (`transactions::create_many`).
- [x] Écran Importer : compte de placement comme destination, aperçu des mouvements, ouverture du portefeuille après l'import.
- [ ] Dividendes : à ajouter avec un relevé qui en contient.

#### V2-12 · Cours de bourse automatiques — M
Décisions du 22/10/2026 : Yahoo Finance (sans clé, non officiel) et saisie manuelle ; consentement explicite dans les Paramètres, désactivé par défaut ; mise à jour quotidienne au démarrage et à la demande ; dépendance `reqwest` (rustls) ; symbole cherché par ISIN ou saisi.
- [x] Table `security_price`, colonne `security.symbol`, table `app_setting` ; récupération des cours (module `investment/prices`, source `investment/yahoo`), cours saisis à la main prioritaires.
- [x] Seule exception au « tout local » : option « Récupérer les cours sur Yahoo Finance » dans les Paramètres, avec ce qui est envoyé et à qui.

#### V2-13 · Performance et plus-values — M
Décisions du 22/10/2026 : un compte avec des mouvements vaut son portefeuille (à partir du premier mouvement), sinon ses valorisations ; titre sans cours au prix de revient ; gain simple (€ et % des achats) et rendement annualisé (TRI) après un an ; plus-value par vente et récapitulatif par année.
- [x] Plus-values latentes et réalisées, dividendes, gain total, rendement annualisé, récapitulatif annuel (`investment::performance`) ; carte Performance et colonnes Valeur et Plus-value latente du portefeuille.
- [x] Patrimoine net et solde des comptes basés sur la valorisation du portefeuille.

### Interface

#### V2-17 · Mode clair et mode sombre — M
Décisions du 22/10/2026 : choix mémorisé dans le stockage local du webview (lu avant le premier affichage, hors du fichier de données) ; palette sombre « violet nuit ».
- [x] Choix « clair », « sombre » ou « système » (par défaut) dans une carte « Apparence » des paramètres ; « système » suit le réglage du système d'exploitation en direct.
- [x] Thème sombre : variables de `src/styles/tokens.css` redéfinies sous `:root[data-theme="dark"]`, couleurs des catégories comprises ; un thème ECharts par mode, options des graphiques recalculées au changement.

### Technique

#### V2-14 · Multi-devises — L
Décisions du 22/10/2026 : euro, devise de référence fixe ; devise portée par le compte (modifiable tant qu'il est vide), les opérations la prennent ; taux saisis à la main ou récupérés avec les cours Yahoo ; flux au taux de leur jour, soldes au taux du jour du solde, montants sans taux comptés zéro avec une alerte ; titres cotés dans une autre devise convertis dans la devise du compte.
- [x] Exploitation des colonnes `currency` (comptes, opérations par déclencheur, titres) ; table `exchange_rate`, module `currency` (taux, `Rates`, `Amounts`).
- [x] Conversion vers la devise de référence dans toutes les statistiques : dépenses par catégorie, revenus et dépenses, soldes, patrimoine net, tiers, plus grosses dépenses, prévision, alertes ; soldes des comptes en euros pour les totaux.
- [x] Cours dans la devise du titre (pence en livres), taux récupérés avec les cours ; virements internes appariés entre comptes de même devise.
- [x] Front : montants affichés dans la devise du compte, devise dans les fiches compte et titre, carte « Devises » des Paramètres, alerte « Taux de change manquant ».

#### V2-15 · Chiffrement du fichier de données — M
Décisions du 22/10/2026 : optionnel, dans les Paramètres ; fichier de données seul (pièces jointes en clair) ; OpenSSL embarqué ; aucune récupération d'un mot de passe oublié, avertissement à l'activation.
- [x] Passage à SQLCipher, mot de passe à l'ouverture (écran de déverrouillage, module `startup`), migration d'un fichier existant par `sqlcipher_export` ; changement de mot de passe et retrait du chiffrement.

#### V2-16 · Sauvegarde complète — M
Décisions du 22/10/2026 : un dossier par sauvegarde (données et pièces jointes) ; au démarrage une fois par jour (désactivable) et à la demande ; 10 complètes et 20 avant import gardées ; dossier au choix, `backups/` par défaut.
- [x] Sauvegardes planifiées, rotation, restauration depuis l'interface (module `backup`, carte « Sauvegardes » des Paramètres) ; restauration appliquée au redémarrage, après une sauvegarde de l'état actuel.

### Corrections

#### V2-21 · Corrections de l'audit du 27/10/2026 — M
- [x] Import QIF : une ligne sans tiers ni mémo (virement Money `L[Livret A]`) prend sa catégorie comme libellé, sinon « Sans libellé », au lieu de faire échouer tout l'import.
- [ ] Modifier la note, les tags ou la date de valeur d'une opération ne réécrit plus la mémoire des libellés : `write_category` seulement si la catégorie change.
- [ ] Import des mouvements titres : un import refusé supprime aussi les titres qu'il a créés.
- [ ] Compteur des sauvegardes avant import : réutiliser `backup::backups_in` au lieu de `batch::backup_count`.
- [ ] Une seule requête `ImportBatch` pour `get`, `list` et `find_by_hash`.
- [ ] Recalcul des libellés normalisés au démarrage seulement quand la normalisation change (version dans `app_setting`).

---

## Décisions à prendre (récapitulatif)

| Ticket | Question |
|---|---|
| V1-02.7 | Lignes d'exemple d'un export Revolut (frais, état) |
