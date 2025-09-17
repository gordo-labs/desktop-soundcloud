import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const qaPlanPath = resolve(__dirname, "../docs/qa-plan.md");
const qaPlan = readFileSync(qaPlanPath, "utf-8");

const escapeForRegex = (value) => value.replace(/[|\\{}()[\]^$+*?.-]/g, "\\$&");

const getSection = (heading) => {
  const regex = new RegExp(
    `${escapeForRegex(heading)}[\\s\\S]*?(?=\\n##\\s|$)`,
    "u"
  );
  const match = qaPlan.match(regex);
  expect(match, `No se encontró la sección "${heading}"`).not.toBeNull();
  return match?.[0] ?? "";
};

const getSubsection = (sectionText, subheading) => {
  const regex = new RegExp(
    `${escapeForRegex(subheading)}[\\s\\S]*?(?=\\n###\\s|\\n##\\s|$)`,
    "u"
  );
  const match = sectionText.match(regex);
  expect(match, `No se encontró la subsección "${subheading}"`).not.toBeNull();
  return match?.[0] ?? "";
};

describe("Documentación del plan de QA", () => {
  it("incluye todos los encabezados principales requeridos", () => {
    const requiredHeadings = [
      "# Plan de QA: SoundCloud Wrapper",
      "## Preparación general",
      "## 1. Arranque en frío y consumo en idle",
      "## 2. Login y persistencia",
      "## 3. Controles de reproducción",
      "## 4. Enlaces externos",
      "## 5. Bandeja del sistema (tray)",
      "## 6. Notificaciones de track",
      "## 7. Integraciones específicas por plataforma",
      "## 8. Seguridad y restricciones",
      "## 9. Cierre y reporte",
    ];

    requiredHeadings.forEach((heading) => {
      expect(qaPlan.includes(heading)).toBe(true);
    });
  });

  it("documenta las plataformas con sus respectivas tareas", () => {
    const platformSection = getSection("## 7. Integraciones específicas por plataforma");
    const linux = getSubsection(platformSection, "### Linux");
    const windows = getSubsection(platformSection, "### Windows");
    const macos = getSubsection(platformSection, "### macOS");

    [linux, windows, macos].forEach((subsection) => {
      expect(subsection.match(/- \[ \]/g)?.length ?? 0).toBeGreaterThan(0);
    });

    expect(linux).toMatch(/MPRIS/);
    expect(windows).toMatch(/SMTC/);
    expect(macos).toMatch(/Now Playing/);
  });

  it("provee checklists accionables para cada etapa", () => {
    const sectionsRequiringChecklist = [
      "## Preparación general",
      "## 1. Arranque en frío y consumo en idle",
      "## 2. Login y persistencia",
      "## 3. Controles de reproducción",
      "## 4. Enlaces externos",
      "## 5. Bandeja del sistema (tray)",
      "## 6. Notificaciones de track",
      "## 7. Integraciones específicas por plataforma",
      "## 8. Seguridad y restricciones",
      "## 9. Cierre y reporte",
    ];

    sectionsRequiringChecklist.forEach((heading) => {
      const section = getSection(heading);
      expect(section.match(/- \[ \]/g)?.length ?? 0).toBeGreaterThan(0);
    });
  });

  it("detalla métricas clave y restricciones de seguridad", () => {
    const coldStartSection = getSection("## 1. Arranque en frío y consumo en idle");
    expect(coldStartSection).toMatch(/<\s*2\s*segundos/);
    expect(coldStartSection).toMatch(/memoria/);

    const securitySection = getSection("## 8. Seguridad y restricciones");
    expect(securitySection).toMatch(/dominio no permitido/);
    expect(securitySection).toMatch(/allowlist/);
    expect(securitySection).toMatch(/Content Security Policy/);
  });
});
