// The plugin's words in English and French, the languages the console speaks.
// Obsidian's own language picks one (main.ts); anything else reads English.

const en = {
  "word.signIn": "sign in",
  "word.update": "update required",
  "word.resyncing": "resyncing",
  "word.confirm": "confirm deletions",
  "word.offline": "offline",
  "word.syncing": "syncing",
  "word.syncingN": "syncing {n}",
  "word.quota": "quota full",
  "word.readOnly": "read-only ({role})",
  "word.conflict": "1 conflict",
  "word.conflicts": "{n} conflicts",
  "word.unsyncable": "unsyncable",
  "word.inSync": "in sync",
  "word.notConfigured": "not configured",

  "when.now": "just now",
  "when.minutes": "{n} min ago",
  "when.hours": "{n} h ago",
  "when.days": "{n} d ago",
  "tip.last": "Last sync {when}",
  "tip.never": "Not synced yet",
  "tip.pending": "{n} local changes to send",

  "menu.sync": "Sync now",
  "menu.problems": "Show problems",
  "menu.openNote": "Open this note in the console",
  "menu.openVault": "Open the vault in the console",
  "menu.history": "Show its history in the console",
  "menu.copyLink": "Copy the Kura link",
  "menu.settings": "Kura settings",
  "menu.signIn": "Sign in",
  "menu.signOut": "Sign out",

  "cmd.sync": "Sync now",
  "cmd.signIn": "Sign in",
  "cmd.signOut": "Sign out",
  "cmd.status": "Show the sync status",
  "cmd.problems": "Show conflicts and unsyncable files",
  "cmd.settings": "Open the settings",
  "cmd.openNote": "Open the current note in the console",
  "cmd.history": "Show the current note's history in the console",
  "cmd.copyLink": "Copy the current note's Kura link",

  "n.copied": "Kura link copied.",
  "n.notSynced": "Kura: this note is not synced yet.",
  "n.conflict": "Kura: a conflict on {name}. Both versions are kept.",
  "n.conflicts": "Kura: {n} new conflicts. Both versions of each are kept.",
  "n.readOnly": "Kura: you are a reader of this vault. Your changes stay on this device.",
  "n.rewound": "Kura: the server lost recent history. Nothing was removed here; files it no longer had were sent again.",
  "n.resync": "Kura: full resync. {n} files deleted elsewhere were moved to the trash.",
  "n.needsObsidian": "Kura Sync needs Obsidian 1.11.4 or later: update Obsidian.",
  "n.setFirst": "Kura: set the server and the identity provider first.",
  "n.https": "Kura: the server and the identity provider must use https://.",
  "n.signedInSyncing": "Signed in to Kura: syncing {vault}.",
  "n.signedInChoose": "Signed in to Kura. Choose a vault in the settings.",
  "n.signInFailed": "Kura: the sign-in did not finish. {cause}",
  "n.configFailed": "Kura: the configuration folder did not sync. {cause} It is tried again in 5 minutes.",
  "n.linkRefused": "Kura: this setup link is refused: {why}.",
  "n.status": "Kura: {word}. {last}",
  "a.open": "Open",
  "a.show": "Show",
  "a.settings": "Settings",

  "welcome.title": "Connect this vault to Kura",
  "welcome.p1": "Kura keeps this vault in sync with your other devices, through your organisation's server.",
  "welcome.p2": "The easiest way: open your Kura console in a browser, pick a vault and press “Open in Obsidian”. Everything is filled in for you.",
  "welcome.p3": "Or enter the server and the identity provider by hand in the settings.",
  "welcome.manual": "Set up by hand",
  "welcome.later": "Later",

  "code.title": "Sign in to Kura",
  "code.step1": "Open the sign-in page and approve this device.",
  "code.prefilled": "The code is already filled in on that page; check that it matches:",
  "code.step2": "Then enter this code:",
  "code.open": "Open the sign-in page",
  "code.copy": "Copy the code",
  "code.copied": "Copied",
  "code.waiting": "Waiting for approval… the code expires in {time}.",
  "code.expired": "The code expired. Close this window and sign in again.",
  "code.cancel": "Cancel",

  "plugin.title": "Kura Sync: a synced plugin",
  "plugin.p1": "The plugin “{name}”{version} was synced from another of your devices.",
  "plugin.p2": "It is installed but disabled on this device. Plugins run code with access to this vault: enable it only if you installed it yourself.",
  "plugin.keep": "Keep disabled",
  "plugin.enable": "Enable",

  "setup.title": "Kura: connect this vault?",
  "setup.p1": "A link asks Kura Sync to use these settings. Accept only if you opened it from your Kura console.",
  "setup.server": "Server: {v}",
  "setup.issuer": "Sign in with: {v}",
  "setup.vault": "Kura vault: {v}",
  "setup.vaultLater": "chosen after signing in",
  "setup.replaces": "This replaces the connection to {v}; this device signs in again.",
  "setup.cancel": "Cancel",
  "setup.connect": "Connect and sign in",

  "trash.title": "Kura: files deleted elsewhere",
  "trash.p1": "This device was away longer than the vault keeps deletions. {n} local files ({share}%) were deleted on other devices. Move them to the trash?",
  "trash.notNow": "Not now",
  "trash.move": "Move to trash",

  "switch.title": "Kura: sync another vault?",
  "switch.p1": "This device stops syncing “{from}” and starts with “{to}”. The files here stay; Kura forgets what it knew of them and compares them with “{to}”.",
  "switch.cancel": "Cancel",
  "switch.go": "Switch vault",

  "problems.title": "Kura: problems",
  "problems.none": "No conflict and no unsyncable file.",
  "problems.conflicts": "Conflicts",
  "problems.conflictsHelp": "Both versions are kept: the other one sits next to the note. Merge them, then delete the copy.",
  "problems.unsyncable": "Unsyncable files",
  "problems.unsyncableHelp": "They stay on this device only.",

  "s.state": "State: {word}",
  "s.connection": "Connection",
  "s.connectedAs": "Signed in to {server}, as the device “{device}”.",
  "s.notSignedIn": "Not signed in.",
  "s.notConfigured": "Not set up yet: use “Open in Obsidian” in your Kura console, or set it up by hand below.",
  "s.signIn": "Sign in",
  "s.signOut": "Sign out",
  "s.manual": "Set up by hand",
  "s.server": "Server",
  "s.issuer": "Identity provider",
  "s.issuerDesc": "The OIDC issuer URL.",
  "s.client": "Client id",
  "s.deviceName": "This device's name",
  "s.deviceNameDesc": "How it shows in “My devices”. Set it before signing in.",
  "s.vault": "Vault",
  "s.vaultDesc": "The Kura vault this Obsidian vault syncs with.",
  "s.vaultNone": "Sign in to choose a vault.",
  "s.role": "{name} (you are {role})",
  "s.openConsole": "Open in the console",
  "s.sync": "Synchronisation",
  "s.syncNow": "Sync now",
  "s.lastNever": "Not synced yet on this device.",
  "s.last": "Last sync {when}: {pushed} sent, {pulled} received.",
  "s.pending": "{n} local changes waiting.",
  "s.advanced": "Advanced",
  "s.maxSize": "Skip files larger than (MiB)",
  "s.maxSizeDesc": "On this device only. Larger files are listed as unsyncable.",
  "s.configSync": "Synced settings",
  "s.configSyncDesc": "Parts of the .obsidian folder the vault shares, set by its managers: {list}.",
  "s.configSyncNone": "none",
  "s.problems": "Problems",

  "role.viewer": "reader",
  "role.editor": "editor",
  "role.manager": "manager",
} as const;

export type Key = keyof typeof en;

const fr: Record<Key, string> = {
  "word.signIn": "connexion requise",
  "word.update": "mise à jour requise",
  "word.resyncing": "resynchronisation",
  "word.confirm": "suppressions à confirmer",
  "word.offline": "hors ligne",
  "word.syncing": "synchronisation",
  "word.syncingN": "synchronisation de {n}",
  "word.quota": "quota plein",
  "word.readOnly": "lecture seule ({role})",
  "word.conflict": "1 conflit",
  "word.conflicts": "{n} conflits",
  "word.unsyncable": "fichiers non synchronisés",
  "word.inSync": "à jour",
  "word.notConfigured": "non configuré",

  "when.now": "à l'instant",
  "when.minutes": "il y a {n} min",
  "when.hours": "il y a {n} h",
  "when.days": "il y a {n} j",
  "tip.last": "Dernière synchro {when}",
  "tip.never": "Pas encore synchronisé",
  "tip.pending": "{n} modifications locales à envoyer",

  "menu.sync": "Synchroniser maintenant",
  "menu.problems": "Voir les problèmes",
  "menu.openNote": "Ouvrir cette note dans la console",
  "menu.openVault": "Ouvrir le coffre dans la console",
  "menu.history": "Voir son historique dans la console",
  "menu.copyLink": "Copier le lien Kura",
  "menu.settings": "Réglages de Kura",
  "menu.signIn": "Se connecter",
  "menu.signOut": "Se déconnecter",

  "cmd.sync": "Synchroniser maintenant",
  "cmd.signIn": "Se connecter",
  "cmd.signOut": "Se déconnecter",
  "cmd.status": "Afficher l'état de la synchro",
  "cmd.problems": "Voir les conflits et les fichiers non synchronisés",
  "cmd.settings": "Ouvrir les réglages",
  "cmd.openNote": "Ouvrir la note courante dans la console",
  "cmd.history": "Voir l'historique de la note courante dans la console",
  "cmd.copyLink": "Copier le lien Kura de la note courante",

  "n.copied": "Lien Kura copié.",
  "n.notSynced": "Kura : cette note n'est pas encore synchronisée.",
  "n.conflict": "Kura : un conflit sur {name}. Les deux versions sont gardées.",
  "n.conflicts": "Kura : {n} nouveaux conflits. Les deux versions de chacun sont gardées.",
  "n.readOnly": "Kura : vous êtes lecteur de ce coffre. Vos modifications restent sur cet appareil.",
  "n.rewound": "Kura : le serveur a perdu un historique récent. Rien n'a été supprimé ici ; les fichiers qu'il n'avait plus ont été renvoyés.",
  "n.resync": "Kura : resynchronisation complète. {n} fichiers supprimés ailleurs ont été mis à la corbeille.",
  "n.needsObsidian": "Kura Sync demande Obsidian 1.11.4 ou plus récent : mettez Obsidian à jour.",
  "n.setFirst": "Kura : renseignez d'abord le serveur et le fournisseur d'identité.",
  "n.https": "Kura : le serveur et le fournisseur d'identité doivent être en https://.",
  "n.signedInSyncing": "Connecté à Kura : synchronisation de {vault}.",
  "n.signedInChoose": "Connecté à Kura. Choisissez un coffre dans les réglages.",
  "n.signInFailed": "Kura : la connexion n'a pas abouti. {cause}",
  "n.configFailed": "Kura : le dossier de configuration ne s'est pas synchronisé. {cause} Nouvel essai dans 5 minutes.",
  "n.linkRefused": "Kura : ce lien de configuration est refusé : {why}.",
  "n.status": "Kura : {word}. {last}",
  "a.open": "Ouvrir",
  "a.show": "Voir",
  "a.settings": "Réglages",

  "welcome.title": "Connecter ce coffre à Kura",
  "welcome.p1": "Kura synchronise ce coffre avec vos autres appareils, par le serveur de votre organisation.",
  "welcome.p2": "Le plus simple : ouvrez votre console Kura dans un navigateur, choisissez un coffre et appuyez sur « Ouvrir dans Obsidian ». Tout est rempli pour vous.",
  "welcome.p3": "Ou saisissez le serveur et le fournisseur d'identité à la main dans les réglages.",
  "welcome.manual": "Configurer à la main",
  "welcome.later": "Plus tard",

  "code.title": "Se connecter à Kura",
  "code.step1": "Ouvrez la page de connexion et approuvez cet appareil.",
  "code.prefilled": "Le code y est déjà rempli ; vérifiez qu'il correspond :",
  "code.step2": "Puis saisissez ce code :",
  "code.open": "Ouvrir la page de connexion",
  "code.copy": "Copier le code",
  "code.copied": "Copié",
  "code.waiting": "En attente d'approbation… le code expire dans {time}.",
  "code.expired": "Le code a expiré. Fermez cette fenêtre et reconnectez-vous.",
  "code.cancel": "Annuler",

  "plugin.title": "Kura Sync : un plugin synchronisé",
  "plugin.p1": "Le plugin « {name} »{version} vient d'un autre de vos appareils.",
  "plugin.p2": "Il est installé mais désactivé sur cet appareil. Un plugin exécute du code avec accès à ce coffre : activez-le seulement si vous l'avez installé vous-même.",
  "plugin.keep": "Laisser désactivé",
  "plugin.enable": "Activer",

  "setup.title": "Kura : connecter ce coffre ?",
  "setup.p1": "Un lien demande à Kura Sync d'utiliser ces réglages. N'acceptez que si vous l'avez ouvert depuis votre console Kura.",
  "setup.server": "Serveur : {v}",
  "setup.issuer": "Connexion avec : {v}",
  "setup.vault": "Coffre Kura : {v}",
  "setup.vaultLater": "choisi après la connexion",
  "setup.replaces": "Cela remplace la connexion à {v} ; cet appareil se reconnecte.",
  "setup.cancel": "Annuler",
  "setup.connect": "Connecter et se connecter",

  "trash.title": "Kura : fichiers supprimés ailleurs",
  "trash.p1": "Cet appareil est resté absent plus longtemps que le coffre ne garde les suppressions. {n} fichiers locaux ({share} %) ont été supprimés sur d'autres appareils. Les mettre à la corbeille ?",
  "trash.notNow": "Pas maintenant",
  "trash.move": "Mettre à la corbeille",

  "switch.title": "Kura : synchroniser un autre coffre ?",
  "switch.p1": "Cet appareil arrête de synchroniser « {from} » et commence avec « {to} ». Les fichiers d'ici restent ; Kura oublie ce qu'il en savait et les compare à « {to} ».",
  "switch.cancel": "Annuler",
  "switch.go": "Changer de coffre",

  "problems.title": "Kura : problèmes",
  "problems.none": "Aucun conflit, aucun fichier non synchronisé.",
  "problems.conflicts": "Conflits",
  "problems.conflictsHelp": "Les deux versions sont gardées : l'autre est à côté de la note. Fusionnez-les, puis supprimez la copie.",
  "problems.unsyncable": "Fichiers non synchronisés",
  "problems.unsyncableHelp": "Ils restent sur cet appareil seulement.",

  "s.state": "État : {word}",
  "s.connection": "Connexion",
  "s.connectedAs": "Connecté à {server}, appareil « {device} ».",
  "s.notSignedIn": "Non connecté.",
  "s.notConfigured": "Pas encore configuré : utilisez « Ouvrir dans Obsidian » dans votre console Kura, ou configurez à la main ci-dessous.",
  "s.signIn": "Se connecter",
  "s.signOut": "Se déconnecter",
  "s.manual": "Configurer à la main",
  "s.server": "Serveur",
  "s.issuer": "Fournisseur d'identité",
  "s.issuerDesc": "L'URL de l'émetteur OIDC.",
  "s.client": "Client id",
  "s.deviceName": "Nom de cet appareil",
  "s.deviceNameDesc": "Tel qu'il apparaît dans « Mes appareils ». À choisir avant de se connecter.",
  "s.vault": "Coffre",
  "s.vaultDesc": "Le coffre Kura avec lequel ce coffre Obsidian se synchronise.",
  "s.vaultNone": "Connectez-vous pour choisir un coffre.",
  "s.role": "{name} (vous êtes {role})",
  "s.openConsole": "Ouvrir dans la console",
  "s.sync": "Synchronisation",
  "s.syncNow": "Synchroniser maintenant",
  "s.lastNever": "Pas encore synchronisé sur cet appareil.",
  "s.last": "Dernière synchro {when} : {pushed} envoyés, {pulled} reçus.",
  "s.pending": "{n} modifications locales en attente.",
  "s.advanced": "Avancé",
  "s.maxSize": "Ignorer les fichiers de plus de (Mio)",
  "s.maxSizeDesc": "Sur cet appareil seulement. Les fichiers plus gros sont listés comme non synchronisés.",
  "s.configSync": "Réglages synchronisés",
  "s.configSyncDesc": "Les parties du dossier .obsidian que le coffre partage, choisies par ses gestionnaires : {list}.",
  "s.configSyncNone": "aucune",
  "s.problems": "Problèmes",

  "role.viewer": "lecteur",
  "role.editor": "éditeur",
  "role.manager": "gestionnaire",
};

let lang: "en" | "fr" = "en";

/** Obsidian's language code: French for any `fr…`, English otherwise. */
export function setLanguage(code: string): void {
  lang = code.toLowerCase().startsWith("fr") ? "fr" : "en";
}

export function language(): "en" | "fr" {
  return lang;
}

/** A word, its `{name}` slots filled. */
export function t(key: Key, vars: Record<string, string | number> = {}): string {
  const text: string = (lang === "fr" ? fr : en)[key];
  return text.replace(/\{(\w+)\}/g, (all, name: string) => (name in vars ? String(vars[name]) : all));
}

/** A role as the console names it; an unknown one as the server sent it. */
export function roleName(role: string): string {
  return role === "viewer" || role === "editor" || role === "manager" ? t(`role.${role}`) : role;
}

/** "3 min ago": a sync time, coarse on purpose. */
export function ago(at: number, now: number): string {
  const s = Math.max(0, Math.round((now - at) / 1000));
  if (s < 60) return t("when.now");
  if (s < 3600) return t("when.minutes", { n: Math.floor(s / 60) });
  if (s < 86_400) return t("when.hours", { n: Math.floor(s / 3600) });
  return t("when.days", { n: Math.floor(s / 86_400) });
}
