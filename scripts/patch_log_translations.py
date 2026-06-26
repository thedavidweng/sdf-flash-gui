#!/usr/bin/env python3
"""Insert log/error L10nKey translations into each language block in i18n.rs."""

from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
I18N = ROOT / "src" / "i18n.rs"

# lang_code -> fn name suffix (t_de, t_fr, ...)
LANG_FN = {
    "bg": "t_bg",
    "hr": "t_hr",
    "cs": "t_cs",
    "da": "t_da",
    "nl": "t_nl",
    "et": "t_et",
    "fi": "t_fi",
    "fr": "t_fr",
    "gl": "t_gl",
    "de": "t_de",
    "el": "t_el",
    "hu": "t_hu",
    "id": "t_id",
    "it": "t_it",
    "lv": "t_lv",
    "lt": "t_lt",
    "ms": "t_ms",
    "nb": "t_nb",
    "pl": "t_pl",
    "pt": "t_pt",
    "pt_br": "t_pt_br",
    "ro": "t_ro",
    "ru": "t_ru",
    "sk": "t_sk",
    "sl": "t_sl",
    "es": "t_es",
    "sv": "t_sv",
    "tr": "t_tr",
    "uk": "t_uk",
}

KEYS = [
    "LogErrGeneric",
    "LogErrLoadManifestBeforeValidate",
    "LogErrSelectImageBeforeValidate",
    "LogFirmwareEmpty",
    "LogFirmwareReadFailed",
    "LogFirmwareLoaded",
    "LogManifestLoaded",
    "LogManifestInvalid",
    "LogManifestReadFailed",
    "LogRecoverSelectWrongFw",
    "LogRecoveryTokenExtracted",
    "LogFileReadFailed",
    "LogValidationFailed",
    "LogProbeResult",
    "LogParsedDrivesFromOutput",
    "LogSdfHeader",
    "LogSdfVendor",
    "LogSdfModel",
    "LogSdfFirmware",
    "LogSdfFlags",
    "LogSdfExtraField",
    "LogSdfReadFailed",
    "ErrMissingToolPath",
    "ErrMissingDrive",
    "ErrUnsupportedPlatform",
    "ErrMissingFirmware",
    "ErrMissingOutputDirectory",
    "ErrMissingRecoveryBootToken",
    "ErrInvalidRecoveryBootToken",
    "ErrConfirmationMismatch",
    "ErrConflictingWriteModes",
    "ErrImageNotFound",
]

# fmt: off
T: dict[str, dict[str, str]] = {
"bg": {
    "LogErrGeneric": "ГРЕШКА: {message}",
    "LogErrLoadManifestBeforeValidate": "ГРЕШКА: заредете манифест преди валидиране",
    "LogErrSelectImageBeforeValidate": "ГРЕШКА: изберете изображение преди валидиране",
    "LogFirmwareEmpty": "ГРЕШКА: файлът с фърмуер е празен: {path}",
    "LogFirmwareReadFailed": "ГРЕШКА: не може да се прочете файлът с фърмуер {path}: {error}",
    "LogFirmwareLoaded": "Зареден фърмуер: {path} ({size} байта, sha256 {hash})",
    "LogManifestLoaded": "Зареден манифест: {vendor} {model} ({count} изображение(я))",
    "LogManifestInvalid": "ГРЕШКА: невалиден манифест: {error}",
    "LogManifestReadFailed": "ГРЕШКА: не може да се прочете манифест: {error}",
    "LogRecoverSelectWrongFw": "ВЪЗСТАНОВЯВАНЕ: изберете грешния файл с фърмуер за извличане на boot token",
    "LogRecoveryTokenExtracted": "Извлечен boot token за възстановяване: {token}",
    "LogFileReadFailed": "ГРЕШКА: не може да се прочете {path}: {error}",
    "LogValidationFailed": "валидирането неуспешно: {error}",
    "LogProbeResult": "MT1959: {mt1959} | Криптиран FW: {encrypted}",
    "LogParsedDrivesFromOutput": "Анализирани {count} устройство(а) от изхода.",
    "LogSdfHeader": "SDF0 v{version} | header_size={header_size} | payload_offset={offset}",
    "LogSdfVendor": "  Производител: {vendor}",
    "LogSdfModel": "  Модел: {model}",
    "LogSdfFirmware": "  Фърмуер: {firmware}",
    "LogSdfFlags": "  Криптиран: {encrypted} | Компресиран: {compressed}",
    "LogSdfExtraField": "  {key}: {value}",
    "LogSdfReadFailed": "ГРЕШКА: не може да се прочете sdf.bin: {error}",
    "ErrMissingToolPath": "Изисква се път до изпълнимия файл на SDFtool",
    "ErrMissingDrive": "Трябва да изберете устройство преди планиране на операция с фърмуер",
    "ErrUnsupportedPlatform": "Избраното устройство не е MT1959",
    "ErrMissingFirmware": "Изисква се път до фърмуер за тази операция",
    "ErrMissingOutputDirectory": "Изисква се изходна папка за дамп на фърмуер",
    "ErrMissingRecoveryBootToken": "Режимът за възстановяване изисква 16-байтов boot token от текущо инсталирания грешен фърмуер",
    "ErrInvalidRecoveryBootToken": "Boot token за възстановяване трябва да е точно 16 печатаеми ASCII байта",
    "ErrConfirmationMismatch": "Несъответствие при потвърждение: въведете „{expected}“ за продължаване",
    "ErrConflictingWriteModes": "Криптиран rawflash и rawflash с boot-loader не могат да се комбинират",
    "ErrImageNotFound": "Изображението с фърмуер не е намерено: {image_id}",
},
"hr": {
    "LogErrGeneric": "GREŠKA: {message}",
    "LogErrLoadManifestBeforeValidate": "GREŠKA: učitajte manifest prije provjere",
    "LogErrSelectImageBeforeValidate": "GREŠKA: odaberite sliku prije provjere",
    "LogFirmwareEmpty": "GREŠKA: datoteka firmwarea je prazna: {path}",
    "LogFirmwareReadFailed": "GREŠKA: nije moguće pročitati datoteku firmwarea {path}: {error}",
    "LogFirmwareLoaded": "Učitan firmware: {path} ({size} bajtova, sha256 {hash})",
    "LogManifestLoaded": "Učitan manifest: {vendor} {model} ({count} slika)",
    "LogManifestInvalid": "GREŠKA: nevažeći manifest: {error}",
    "LogManifestReadFailed": "GREŠKA: nije moguće pročitati manifest: {error}",
    "LogRecoverSelectWrongFw": "OPORAVAK: odaberite pogrešnu datoteku firmwarea za izdvajanje boot tokena",
    "LogRecoveryTokenExtracted": "Izdvojen boot token za oporavak: {token}",
    "LogFileReadFailed": "GREŠKA: nije moguće pročitati {path}: {error}",
    "LogValidationFailed": "provjera nije uspjela: {error}",
    "LogProbeResult": "MT1959: {mt1959} | Šifrirani FW: {encrypted}",
    "LogParsedDrivesFromOutput": "Parsirano {count} pogona iz izlaza.",
    "LogSdfHeader": "SDF0 v{version} | header_size={header_size} | payload_offset={offset}",
    "LogSdfVendor": "  Proizvođač: {vendor}",
    "LogSdfModel": "  Model: {model}",
    "LogSdfFirmware": "  Firmware: {firmware}",
    "LogSdfFlags": "  Šifrirano: {encrypted} | Komprimirano: {compressed}",
    "LogSdfExtraField": "  {key}: {value}",
    "LogSdfReadFailed": "GREŠKA: nije moguće pročitati sdf.bin: {error}",
    "ErrMissingToolPath": "Potreban je put do izvršne datoteke SDFtool",
    "ErrMissingDrive": "Morate odabrati pogon prije planiranja operacije s firmwareom",
    "ErrUnsupportedPlatform": "Odabrani pogon nije MT1959",
    "ErrMissingFirmware": "Za ovu operaciju potreban je put do firmwarea",
    "ErrMissingOutputDirectory": "Za dump firmwarea potreban je izlazni direktorij",
    "ErrMissingRecoveryBootToken": "Način oporavka zahtijeva 16-bajtni boot token iz trenutno instaliranog pogrešnog firmwarea",
    "ErrInvalidRecoveryBootToken": "Boot token za oporavak mora imati točno 16 ispisivih ASCII bajtova",
    "ErrConfirmationMismatch": "Nepodudaranje potvrde: upišite „{expected}“ za nastavak",
    "ErrConflictingWriteModes": "Šifrirani rawflash i rawflash s boot-loaderom ne mogu se kombinirati",
    "ErrImageNotFound": "Slika firmwarea nije pronađena: {image_id}",
},
"cs": {
    "LogErrGeneric": "CHYBA: {message}",
    "LogErrLoadManifestBeforeValidate": "CHYBA: před ověřením načtěte manifest",
    "LogErrSelectImageBeforeValidate": "CHYBA: před ověřením vyberte obraz",
    "LogFirmwareEmpty": "CHYBA: soubor firmwaru je prázdný: {path}",
    "LogFirmwareReadFailed": "CHYBA: nelze přečíst soubor firmwaru {path}: {error}",
    "LogFirmwareLoaded": "Načten firmware: {path} ({size} bajtů, sha256 {hash})",
    "LogManifestLoaded": "Načten manifest: {vendor} {model} ({count} obraz(ů))",
    "LogManifestInvalid": "CHYBA: neplatný manifest: {error}",
    "LogManifestReadFailed": "CHYBA: nelze přečíst manifest: {error}",
    "LogRecoverSelectWrongFw": "OBNOVA: vyberte špatný soubor firmwaru pro extrakci boot tokenu",
    "LogRecoveryTokenExtracted": "Extrahován boot token pro obnovu: {token}",
    "LogFileReadFailed": "CHYBA: nelze přečíst {path}: {error}",
    "LogValidationFailed": "ověření selhalo: {error}",
    "LogProbeResult": "MT1959: {mt1959} | Šifrovaný FW: {encrypted}",
    "LogParsedDrivesFromOutput": "Z výstupu bylo analyzováno {count} jednotek.",
    "LogSdfHeader": "SDF0 v{version} | header_size={header_size} | payload_offset={offset}",
    "LogSdfVendor": "  Výrobce: {vendor}",
    "LogSdfModel": "  Model: {model}",
    "LogSdfFirmware": "  Firmware: {firmware}",
    "LogSdfFlags": "  Šifrováno: {encrypted} | Komprimováno: {compressed}",
    "LogSdfExtraField": "  {key}: {value}",
    "LogSdfReadFailed": "CHYBA: nelze přečíst sdf.bin: {error}",
    "ErrMissingToolPath": "Je vyžadována cesta k spustitelnému souboru SDFtool",
    "ErrMissingDrive": "Před plánováním operace s firmwarem musíte vybrat jednotku",
    "ErrUnsupportedPlatform": "Vybraná jednotka není MT1959",
    "ErrMissingFirmware": "Pro tuto operaci je vyžadována cesta k firmwaru",
    "ErrMissingOutputDirectory": "Pro dump firmwaru je vyžadován výstupní adresář",
    "ErrMissingRecoveryBootToken": "Režim obnovy vyžaduje 16bajtový boot token z aktuálně nainstalovaného špatného firmwaru",
    "ErrInvalidRecoveryBootToken": "Boot token pro obnovu musí mít přesně 16 tisknutelných ASCII bajtů",
    "ErrConfirmationMismatch": "Neshoda potvrzení: pro pokračování zadejte „{expected}“",
    "ErrConflictingWriteModes": "Šifrovaný rawflash a rawflash s boot-loaderem nelze kombinovat",
    "ErrImageNotFound": "Obraz firmwaru nenalezen: {image_id}",
},
"da": {
    "LogErrGeneric": "FEJL: {message}",
    "LogErrLoadManifestBeforeValidate": "FEJL: indlæs et manifest før validering",
    "LogErrSelectImageBeforeValidate": "FEJL: vælg et billede før validering",
    "LogFirmwareEmpty": "FEJL: firmwarefilen er tom: {path}",
    "LogFirmwareReadFailed": "FEJL: kan ikke læse firmwarefil {path}: {error}",
    "LogFirmwareLoaded": "Firmware indlæst: {path} ({size} bytes, sha256 {hash})",
    "LogManifestLoaded": "Manifest indlæst: {vendor} {model} ({count} billede(r))",
    "LogManifestInvalid": "FEJL: ugyldigt manifest: {error}",
    "LogManifestReadFailed": "FEJL: kan ikke læse manifest: {error}",
    "LogRecoverSelectWrongFw": "GENDAN: vælg den forkerte firmwarefil for at udtrække boot-token",
    "LogRecoveryTokenExtracted": "Gendannelses-boot-token udtrukket: {token}",
    "LogFileReadFailed": "FEJL: kan ikke læse {path}: {error}",
    "LogValidationFailed": "validering mislykkedes: {error}",
    "LogProbeResult": "MT1959: {mt1959} | Krypteret FW: {encrypted}",
    "LogParsedDrivesFromOutput": "Parset {count} drev fra output.",
    "LogSdfHeader": "SDF0 v{version} | header_size={header_size} | payload_offset={offset}",
    "LogSdfVendor": "  Producent: {vendor}",
    "LogSdfModel": "  Model: {model}",
    "LogSdfFirmware": "  Firmware: {firmware}",
    "LogSdfFlags": "  Krypteret: {encrypted} | Komprimeret: {compressed}",
    "LogSdfExtraField": "  {key}: {value}",
    "LogSdfReadFailed": "FEJL: kan ikke læse sdf.bin: {error}",
    "ErrMissingToolPath": "Sti til SDFtool-kørbar fil er påkrævet",
    "ErrMissingDrive": "Et drev skal vælges før planlægning af en firmwareoperation",
    "ErrUnsupportedPlatform": "Det valgte drev er ikke et MT1959-drev",
    "ErrMissingFirmware": "Firmwaresti er påkrævet for denne operation",
    "ErrMissingOutputDirectory": "Outputmappe er påkrævet for firmware-dump",
    "ErrMissingRecoveryBootToken": "Gendannelsestilstand kræver et 16-byte boot-token fra den aktuelt installerede forkerte firmware",
    "ErrInvalidRecoveryBootToken": "Gendannelses-boot-token skal være præcis 16 udskrivbare ASCII-bytes",
    "ErrConfirmationMismatch": "Bekræftelses-uoverensstemmelse: skriv „{expected}“ for at fortsætte",
    "ErrConflictingWriteModes": "Krypteret rawflash og boot-loader rawflash kan ikke kombineres",
    "ErrImageNotFound": "Firmwarebillede ikke fundet: {image_id}",
},
"nl": {
    "LogErrGeneric": "FOUT: {message}",
    "LogErrLoadManifestBeforeValidate": "FOUT: laad een manifest voordat u valideert",
    "LogErrSelectImageBeforeValidate": "FOUT: selecteer een image voordat u valideert",
    "LogFirmwareEmpty": "FOUT: firmwarebestand is leeg: {path}",
    "LogFirmwareReadFailed": "FOUT: kan firmwarebestand {path} niet lezen: {error}",
    "LogFirmwareLoaded": "Firmware geladen: {path} ({size} bytes, sha256 {hash})",
    "LogManifestLoaded": "Manifest geladen: {vendor} {model} ({count} image(s))",
    "LogManifestInvalid": "FOUT: ongeldig manifest: {error}",
    "LogManifestReadFailed": "FOUT: kan manifest niet lezen: {error}",
    "LogRecoverSelectWrongFw": "HERSTEL: selecteer het verkeerde firmwarebestand om boot-token te extraheren",
    "LogRecoveryTokenExtracted": "Herstel-boot-token geëxtraheerd: {token}",
    "LogFileReadFailed": "FOUT: kan {path} niet lezen: {error}",
    "LogValidationFailed": "validatie mislukt: {error}",
    "LogProbeResult": "MT1959: {mt1959} | Versleutelde FW: {encrypted}",
    "LogParsedDrivesFromOutput": "{count} station(s) uit output geparsed.",
    "LogSdfHeader": "SDF0 v{version} | header_size={header_size} | payload_offset={offset}",
    "LogSdfVendor": "  Fabrikant: {vendor}",
    "LogSdfModel": "  Model: {model}",
    "LogSdfFirmware": "  Firmware: {firmware}",
    "LogSdfFlags": "  Versleuteld: {encrypted} | Gecomprimeerd: {compressed}",
    "LogSdfExtraField": "  {key}: {value}",
    "LogSdfReadFailed": "FOUT: kan sdf.bin niet lezen: {error}",
    "ErrMissingToolPath": "Pad naar SDFtool-uitvoerbaar bestand is vereist",
    "ErrMissingDrive": "Er moet een station worden geselecteerd voordat een firmwareoperatie wordt gepland",
    "ErrUnsupportedPlatform": "Geselecteerd station is geen MT1959-station",
    "ErrMissingFirmware": "Firmwarepad is vereist voor deze operatie",
    "ErrMissingOutputDirectory": "Uitvoermap is vereist voor firmware-dump",
    "ErrMissingRecoveryBootToken": "Herstelmodus vereist een 16-byte boot-token van de huidig geïnstalleerde verkeerde firmware",
    "ErrInvalidRecoveryBootToken": "Herstel-boot-token moet precies 16 afdrukbare ASCII-bytes zijn",
    "ErrConfirmationMismatch": "Bevestiging komt niet overeen: typ „{expected}“ om door te gaan",
    "ErrConflictingWriteModes": "Versleutelde rawflash en boot-loader rawflash kunnen niet worden gecombineerd",
    "ErrImageNotFound": "Firmware-image niet gevonden: {image_id}",
},
}
# fmt: on

# Remaining languages appended in second write due to size — loaded from companion file if present.
COMPANION = Path(__file__).with_name("patch_log_translations_data.py")

def rust_line(key: str, value: str) -> str:
    escaped = value.replace('"', '\\"')
    return f'    L10nKey::{key} => r#"{escaped}"#,'

def block_for_lang(lang: str) -> str:
    data = T[lang]
    missing = [k for k in KEYS if k not in data]
    if missing:
        raise SystemExit(f"Missing keys for {lang}: {missing}")
    return "\n".join(rust_line(k, data[k]) for k in KEYS)

def patch_lang(text: str, fn_name: str, lang: str) -> str:
    marker = f"L10nKey::LogErrGeneric"
    if marker in text.split(f"translations! {{ fn {fn_name} {{")[1].split("} }")[0]:
        print(f"skip {lang}: already patched")
        return text

    pattern = (
        rf"(translations! \{{ fn {re.escape(fn_name)} \{{[\s\S]*?"
        rf"L10nKey::ThemeLight => r#\"[^\"]*\"#,)\s*\n(\s*\}} \}})"
    )
    replacement = rf"\1\n{block_for_lang(lang)}\n\2"
    new_text, n = re.subn(pattern, replacement, text, count=1)
    if n != 1:
        raise SystemExit(f"Failed to patch {fn_name} ({lang}), matches={n}")
    print(f"patched {lang}")
    return new_text

def main() -> None:
    if COMPANION.exists():
        ns: dict = {}
        exec(COMPANION.read_text(encoding="utf-8"), ns)
        T.update(ns.get("T_EXTRA", {}))

    text = I18N.read_text(encoding="utf-8")
    for lang, fn in LANG_FN.items():
        if lang not in T:
            raise SystemExit(f"No translations for {lang}")
        text = patch_lang(text, fn, lang)
    I18N.write_text(text, encoding="utf-8")
    print("done")

if __name__ == "__main__":
    main()