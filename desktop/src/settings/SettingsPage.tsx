import { useState } from "react";
import TruthIdSettingsSection from "../truthid/TruthIdSettingsSection";
import FinanceApiSettingsSection from "./FinanceApiSettingsSection";
import StorageSettingsSection from "./StorageSettingsSection";
import BackupSettingsSection from "./BackupSettingsSection";
import TaxSettingsSection from "./TaxSettingsSection";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

// Fase 20 (Sessão 101): a seção "IA" (chaves de API Gemini/Claude/OpenAI) saiu
// junto com toda a IA do Anchor — o Warden virou a única interface de IA do
// ecossistema.
// "TruthID" migrou pra cá na Sessão 54 (era aba própria no nav principal,
// "TruthID Sync") — pedido explícito do dono do projeto. Confirma que o
// layout de sidebar + conteúdo já pensado pra caber mais de uma seção
// (comentário original, quando só "IA" existia) funcionou sem redesenho;
// ainda sem abstração de registro de seções — só 2 itens, não vale a pena.
const SECTIONS = ["TruthID", "Finance API", "Storage", "Backup", "Taxes"] as const;

function SettingsPage({ onBack }: { onBack: () => void }) {
  const [section, setSection] = useState<(typeof SECTIONS)[number]>("TruthID");

  return (
    <Card>
      <CardHeader>
        <div className="flex items-center gap-3">
          <Button variant="outline" size="sm" onClick={onBack}>
            ← Back
          </Button>
          <CardTitle>Settings</CardTitle>
        </div>
      </CardHeader>
      <CardContent className="flex gap-6">
        <nav className="flex w-40 shrink-0 flex-col gap-1">
          {SECTIONS.map((s) => (
            <button
              key={s}
              type="button"
              onClick={() => setSection(s)}
              className={`rounded-md px-3 py-2 text-left text-sm ${
                section === s
                  ? "bg-muted font-medium"
                  : "text-muted-foreground hover:bg-muted/50"
              }`}
            >
              {s}
            </button>
          ))}
        </nav>
        <div className="flex-1">
          {section === "TruthID" && <TruthIdSettingsSection />}
          {section === "Finance API" && <FinanceApiSettingsSection />}
          {section === "Storage" && <StorageSettingsSection />}
          {section === "Backup" && <BackupSettingsSection />}
          {section === "Taxes" && <TaxSettingsSection />}
        </div>
      </CardContent>
    </Card>
  );
}

export default SettingsPage;
