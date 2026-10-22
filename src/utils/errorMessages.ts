import type {
  AccountError,
  AttachmentError,
  BackupError,
  CategoryError,
  CsvError,
  CsvMappingError,
  ErrorDetail,
  ImportError,
  InvestmentError,
  OperationError,
  PayeeError,
  PdfError,
  PriceError,
  RuleError,
  ScheduleError,
  StatsError,
  TagError,
  TransferError,
} from "../bindings";
import { formatDate, formatFileSize } from "./format";

/**
 * French messages of the business errors. Each switch covers every code: a code added on the
 * backend without its message fails the type check (see `unreachable`).
 */

function unreachable(value: never): never {
  throw new Error(`unknown error ${JSON.stringify(value)}`);
}

function account(error: AccountError): string {
  switch (error.code) {
    case "notFound":
      return "Ce compte n'existe plus.";
    case "emptyName":
      return "Le nom du compte est obligatoire.";
    case "valuationNotFound":
      return "Cette valorisation n'existe plus.";
    case "notValued":
      return "Les comptes courants, d'épargne et les cartes se suivent par leurs opérations, pas par des valorisations.";
    case "negativeValuation":
      return "Une valorisation ne peut pas être négative : pour une dette, saisis le capital restant dû.";
    case "duplicateValuationDate":
      return `Ce compte a déjà une valorisation au ${formatDate(error.params)}.`;
    case "valuationBeforeOpening":
      return `Une valorisation ne peut pas précéder l'ouverture du compte (${formatDate(error.params)}).`;
  }
  return unreachable(error);
}

function operation(error: OperationError): string {
  switch (error.code) {
    case "notFound":
      return "Cette opération n'existe plus.";
    case "categoryNotFound":
      return "Cette catégorie n'existe plus.";
    case "emptyLabel":
      return "Le libellé est obligatoire.";
    case "zeroAmount":
      return "Le montant ne peut pas être nul.";
    case "accountWithoutOperations":
      return "Les placements, actifs et dettes n'ont pas d'opérations : ils se suivent par des valorisations.";
    case "dateBeforeOpening":
      return `La date ne peut pas précéder l'ouverture du compte (${formatDate(error.params)}) : ces opérations sont déjà comprises dans le solde d'ouverture.`;
    case "importedFieldsLocked":
      return "La date, le montant et le libellé d'une opération importée ne sont pas modifiables.";
    case "importedOperationNotDeletable":
      return "Une opération importée ne se supprime pas seule : annule son import.";
  }
  return unreachable(error);
}

function category(error: CategoryError): string {
  switch (error.code) {
    case "notFound":
      return "Cette catégorie n'existe plus.";
    case "emptyName":
      return "Le nom de la catégorie est obligatoire.";
    case "duplicateName":
      return `Une catégorie « ${error.params} » existe déjà à cet endroit.`;
    case "systemCategoryLocked":
      return "La catégorie « Virement interne » ne peut être ni modifiée ni supprimée.";
    case "invalidParent":
      return "Une sous-catégorie se place sous une catégorie principale, autre que « Virement interne ».";
    case "hasSubcategories":
      return "Cette catégorie a des sous-catégories : déplace-les ou supprime-les d'abord.";
    case "reassignToSelf":
      return "Choisis une autre catégorie pour reprendre les opérations.";
  }
  return unreachable(error);
}

function schedule(error: ScheduleError): string {
  switch (error.code) {
    case "notFound":
      return "Cette échéance n'existe plus.";
    case "emptyLabel":
      return "Le libellé est obligatoire.";
    case "zeroAmount":
      return "Le montant ne peut pas être nul.";
    case "invalidDay":
      return "Le jour doit être un jour de la semaine pour une échéance hebdomadaire, un jour de 1 à 31 sinon.";
    case "endBeforeStart":
      return "La date de fin ne peut pas précéder la date de début.";
    case "accountWithoutOperations":
      return "Une échéance se rattache à un compte courant, d'épargne ou à une carte.";
    case "categoryNotFound":
      return "Cette catégorie n'existe plus.";
    case "notAnOccurrence":
      return `Le ${formatDate(error.params)} n'est pas une date de cette échéance.`;
    case "otherAccount":
      return "L'opération n'est pas sur le compte de l'échéance.";
    case "alreadyRealized":
      return "Cette échéance est déjà réalisée par une autre opération.";
    case "operationAlreadyMatched":
      return "Cette opération réalise déjà une autre échéance.";
  }
  return unreachable(error);
}

function rule(error: RuleError): string {
  switch (error.code) {
    case "notFound":
      return "Cette règle n'existe plus.";
    case "emptyPattern":
      return "Indique le texte que le libellé doit contenir.";
    case "categoryNotFound":
      return "Cette catégorie n'existe plus.";
    case "invalidOrder":
      return "La liste des règles a changé entre-temps : recharge l'écran.";
  }
  return unreachable(error);
}

function payee(error: PayeeError): string {
  switch (error.code) {
    case "notFound":
      return "Ce tiers n'existe plus.";
    case "labelNotFound":
      return `Le libellé « ${error.params} » n'appartient plus à aucun tiers.`;
    case "emptyName":
      return "Le nom du tiers est obligatoire.";
    case "nameTaken":
      return `Un tiers s'appelle déjà « ${error.params.name} ».`;
    case "mergeIntoItself":
      return "Un tiers ne peut pas être fusionné avec lui-même.";
    case "lastLabel":
      return "C'est le seul libellé de ce tiers : il ne peut pas en être détaché.";
  }
  return unreachable(error);
}

function tag(error: TagError): string {
  switch (error.code) {
    case "notFound":
      return "Ce tag n'existe plus.";
    case "emptyName":
      return "Le nom du tag est obligatoire.";
    case "nameTaken":
      return `Un tag s'appelle déjà « ${error.params} ».`;
  }
  return unreachable(error);
}

function attachment(error: AttachmentError): string {
  switch (error.code) {
    case "notFound":
      return "Cette pièce jointe n'existe plus.";
    case "notAFile":
      return "Seul un fichier peut être joint.";
    case "tooLarge":
      return `Le fichier dépasse ${formatFileSize(error.params.maxBytes)}.`;
    case "fileMissing":
      return "Le fichier de cette pièce jointe a disparu du dossier des données.";
  }
  return unreachable(error);
}

function transfer(error: TransferError): string {
  switch (error.code) {
    case "sameAccount":
      return "Les deux côtés d'un virement interne doivent être sur des comptes différents.";
    case "amountsNotOpposite":
      return "Les deux côtés d'un virement interne doivent avoir des montants opposés.";
    case "alreadyLinked":
      return "Une de ces opérations est déjà liée à un autre virement.";
  }
  return unreachable(error);
}

function stats(error: StatsError): string {
  switch (error.code) {
    case "invalidPeriod":
      return `La période se termine (${formatDate(error.params.to)}) avant de commencer (${formatDate(error.params.from)}).`;
  }
  return unreachable(error.code);
}

function csv(error: CsvError): string {
  switch (error.code) {
    case "invalidDelimiter":
      return "Le séparateur de colonnes doit être un caractère simple (; , | ou tabulation).";
    case "emptyDateFormat":
      return "Le format des dates est obligatoire.";
    case "invalidDateFormat":
      return `Le format de date « ${error.params} » n'est pas valide.`;
    case "emptyStateValue":
      return "Indique l'état des opérations à importer (par exemple COMPLETED).";
    case "invalidEncoding":
      return `Le fichier n'est pas encodé en ${error.params} : choisis un autre encodage dans le format.`;
    case "malformed":
      return `Ligne ${error.params.line} : ligne CSV mal formée.`;
    case "missingColumn":
      return `Ligne ${error.params.line} : la colonne ${error.params.column + 1} manque.`;
    case "invalidDate":
      return `Ligne ${error.params.line} : date « ${error.params.value} » illisible avec ce format.`;
    case "invalidAmount":
      return `Ligne ${error.params.line} : montant « ${error.params.value} » illisible.`;
    case "wrongDecimalSeparator":
      return `Ligne ${error.params.line} : le montant « ${error.params.value} » utilise l'autre séparateur décimal : change le « Séparateur décimal » du format.`;
    case "missingAmount":
      return `Ligne ${error.params.line} : aucun montant.`;
    case "bothDebitAndCredit":
      return `Ligne ${error.params.line} : un débit et un crédit à la fois.`;
    case "emptyLabel":
      return `Ligne ${error.params.line} : libellé vide.`;
  }
  return unreachable(error);
}

function csvMapping(error: CsvMappingError): string {
  switch (error.code) {
    case "notFound":
      return "Ce format n'existe plus.";
    case "emptyName":
      return "Le nom du format est obligatoire.";
    case "duplicateName":
      return `Un format « ${error.params} » existe déjà.`;
    case "corrupt":
      return "Ce format enregistré est illisible : supprime-le et recrée-le.";
  }
  return unreachable(error);
}

function importing(error: ImportError): string {
  switch (error.code) {
    case "notFound":
      return "Cet import n'existe plus.";
    case "noLine":
      return "Aucune opération à importer : coche au moins une ligne.";
    case "invalidLine":
      return "Une ligne du relevé a un libellé vide ou un montant nul.";
    case "fileChanged":
      return "Le fichier a changé depuis l'aperçu : recommence l'import.";
    case "lineOutOfRange":
      return `La ligne ${error.params + 1} de l'aperçu n'existe pas : recommence l'import.`;
    case "duplicateLine":
      return `La ligne ${error.params + 1} est déjà dans le compte et ne peut pas être réimportée.`;
  }
  return unreachable(error);
}

function pdf(error: PdfError): string {
  switch (error.code) {
    case "libraryUnavailable":
      return "La bibliothèque de lecture des PDF (PDFium) est introuvable : réinstalle l'application.";
    case "invalidPdf":
      return "Ce fichier n'est pas un PDF lisible.";
    case "unknownFormat":
      return "Ce PDF n'est pas un relevé d'une banque prise en charge.";
    case "layout":
      return `La mise en page de ce relevé n'est pas celle attendue (${error.params}).`;
  }
  return unreachable(error);
}

function investment(error: InvestmentError): string {
  switch (error.code) {
    case "securityNotFound":
      return "Ce titre n'existe plus.";
    case "transactionNotFound":
      return "Ce mouvement n'existe plus.";
    case "emptySecurityName":
      return "Le nom du titre est obligatoire.";
    case "invalidIsin":
      return `« ${error.params} » n'est pas un code ISIN valide (2 lettres, 9 caractères et un chiffre de contrôle).`;
    case "invalidSymbol":
      return `« ${error.params} » n'est pas un symbole valide (lettres, chiffres, point ou tiret, par exemple CW8.PA).`;
    case "isinTaken":
      return `Un titre a déjà le code ISIN ${error.params}.`;
    case "securityInUse":
      return "Ce titre a des mouvements : supprime-les d'abord.";
    case "notAnInvestmentAccount":
      return "Seuls les comptes de placement ont un portefeuille.";
    case "dateBeforeOpening":
      return `La date précède l'ouverture du compte (${formatDate(error.params)}).`;
    case "missingQuantity":
      return "Un achat ou une vente demande une quantité.";
    case "quantityOnDividend":
      return "Un dividende n'a pas de quantité.";
    case "zeroAmount":
      return "Le montant doit être positif.";
    case "oversold":
      return `Le ${formatDate(error.params.date)}, la vente dépasserait la quantité détenue de ${error.params.security}.`;
  }
  return unreachable(error);
}

export function priceMessage(error: PriceError): string {
  switch (error.code) {
    case "disabled":
      return "La récupération des cours est désactivée : active-la dans les Paramètres.";
    case "network":
      return `Yahoo Finance est injoignable (${error.params}). Vérifie la connexion et réessaie.`;
    case "unexpectedResponse":
      return "Yahoo Finance a répondu dans un format inattendu : réessaie plus tard, ou saisis le cours à la main.";
    case "unknownSymbol":
      return `Le symbole ${error.params} est inconnu de Yahoo Finance.`;
    case "unsupportedCurrency":
      return `${error.params.symbol} est coté en ${error.params.currency} : seuls les cours en euros sont pris en charge.`;
    case "invalidPrice":
      return "Le cours doit être positif.";
  }
  return unreachable(error);
}

function backup(error: BackupError): string {
  switch (error.code) {
    case "notABackup":
      return `${error.params} n'est pas une sauvegarde de l'application.`;
    case "folderUnavailable":
      return `Le dossier ${error.params} est inaccessible en écriture : choisis-en un autre.`;
  }
  return unreachable(error);
}

/** French message of a business error. */
export function businessMessage(detail: ErrorDetail): string {
  switch (detail.domain) {
    case "account":
      return account(detail.error);
    case "operation":
      return operation(detail.error);
    case "category":
      return category(detail.error);
    case "schedule":
      return schedule(detail.error);
    case "rule":
      return rule(detail.error);
    case "payee":
      return payee(detail.error);
    case "tag":
      return tag(detail.error);
    case "attachment":
      return attachment(detail.error);
    case "transfer":
      return transfer(detail.error);
    case "stats":
      return stats(detail.error);
    case "csv":
      return csv(detail.error);
    case "csvMapping":
      return csvMapping(detail.error);
    case "import":
      return importing(detail.error);
    case "pdf":
      return pdf(detail.error);
    case "investment":
      return investment(detail.error);
    case "price":
      return priceMessage(detail.error);
    case "backup":
      return backup(detail.error);
  }
  return unreachable(detail);
}
