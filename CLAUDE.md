# CLAUDE.md

Instructions pour Claude Code sur ce projet.

## Contexte

Application de bureau de finances personnelles (Tauri 2 + Rust + Vue 3), locale et mono-utilisateur, inspirée de Microsoft Money 2000.

**`README.md` est la spécification de référence** : philosophie, périmètre, roadmap V1 / V2 / plus tard, règles de calcul, modèle de données, flux d'import, stack, structure du projet et ordre de développement. Lis-le en entier avant toute tâche, et respecte-le. Si une demande le contredit, signale-le avant d'agir.

## Langue

- Réponds-moi en **français**.
- Écris le **code, les commentaires, les noms et les messages de commit en anglais**.

## Règles du projet

- **Périmètre** : implémente uniquement la version en cours (V1 sauf indication contraire). N'ajoute rien de la section « Hors périmètre ». Les colonnes notées *(prévu)* dans le modèle de données sont créées dès la V1, mais restent inexploitées.
- **Stack** : respecte la stack du README. Demande-moi avant d'ajouter une dépendance qui n'y figure pas.
- **Montants** : toujours en `i64` centimes, jamais de `f32`/`f64`. Le signe est négatif pour un débit et positif pour un crédit.
- **Catégorie** : elle est portée par `operation_split`, même s'il n'y a qu'une ligne par opération en V1. Toutes les requêtes de statistiques passent par cette table.
- **Virements internes** : exclus de tous les indicateurs de revenus et dépenses, mais inclus dans les soldes.
- **Libellés** : `raw_label` n'est jamais modifié ; `normalized_label` sert à la catégorisation et aux tiers.
- **Architecture Rust** : les `#[tauri::command]` dans `commands/` restent une couche fine qui délègue aux modules métier. Les erreurs métier sont typées avec `thiserror`. Les requêtes de `stats/` ont des signatures claires, car elles serviront d'outils à l'assistant IA plus tard.
- **Front** : Vue 3 + TypeScript (Composition API, `<script setup>`). Les appels au backend passent uniquement par `src/bindings.ts`, généré par tauri-specta, qu'on ne modifie jamais à la main.
- **Tests** : tests unitaires Rust sur la normalisation des libellés, la détection des doublons, le contrôle des soldes et les agrégations. Les relevés de test sont des fichiers anonymisés dans `tests/fixtures/`.

## Design

Les maquettes validées sont dans `docs/design/` :

| Fichier | Écran |
|---|---|
| `Dashboard.dc.html` | Tableau de bord |
| `Operations.dc.html` | Liste des opérations d'un compte |
| `Import.dc.html` | Import de relevé (étape vérification et aperçu) |
| `Analyses.dc.html` | Analyses et graphiques |

Ces fichiers sont dans un format de maquette spécifique (`.dc.html`) : ne les exécute pas et ne les copie pas tels quels. Utilise-les comme **référence visuelle** :
- la structure et les styles sont dans le balisage, en styles inline ;
- les données d'exemple et certains styles calculés (pastilles de catégorie, lignes de tableau) sont dans la fonction `renderVals()` du bloc `<script type="text/x-dc">` en bas de chaque fichier ;
- les graphiques de la maquette sont des SVG ou des barres CSS statiques : dans l'application, ils doivent être réalisés avec **ECharts** (vue-echarts), avec le même rendu.

Reproduis le design avec des **variables CSS partagées** (`src/styles/tokens.css`) plutôt que des couleurs en dur :

| Rôle | Valeur |
|---|---|
| Police | Plus Jakarta Sans (400–800), chiffres tabulaires |
| Fond de page | `#F5F3F9` |
| Cartes | `#FFFFFF`, bordure `#E7E3EF`, rayon 18px |
| Barre latérale et bouton principal | `#2A1E3F` |
| Texte / texte secondaire | `#1E1830` / `#6B6480` |
| Accent | `#F26B3A` (corail) |
| Montant positif | `#0F7A5F` |

Couleurs des catégories (couleur pleine pour les graphiques / fond clair de la pastille / texte de la pastille) :

| Catégorie | Pleine | Fond | Texte |
|---|---|---|---|
| Logement | `#5B6CF0` | `#E5E8FD` | `#3341B8` |
| Courses | `#F2A93B` | `#FDF0D9` | `#8A5A0B` |
| Restaurants | `#F26B3A` | `#FDE7DE` | `#9A3412` |
| Transport, revenus | `#1F9E89` | `#DDF3EF` | `#0F5E52` |
| Loisirs | `#E8547A` | `#FCE3EA` | `#A61E48` |
| Abonnements | `#3BA3D9` | `#E0F1FA` | `#1B6A94` |
| Santé | `#8C5AD8` | `#EEE5FA` | `#5B2CA3` |
| Virement interne, autres | `#B9B4C6` | `#EEEDF2` | `#4A4657` |

La police doit être **embarquée dans l'application** (fichiers `.woff2` dans le projet), pas chargée depuis Google Fonts : l'application fonctionne hors ligne.

## Façon de travailler

- Avance **étape par étape** en suivant « Ordre de développement V1 » du README. Termine et vérifie une étape avant de passer à la suivante.
- Pour chaque étape, propose d'abord un plan court, puis implémente.
- Après chaque modification : `cargo check`, `cargo test` et `cargo clippy` dans `src-tauri/`, puis `npm run build` pour le front.
- Fais des commits petits et cohérents, un par sous-tâche.
- Si une décision n'est pas couverte par le README, pose-moi la question au lieu de deviner. Quand une décision est prise, propose de l'ajouter au README.
