import { useEffect, useState } from "react";
import { LogOut, PanelTopClose } from "lucide-react";
import { useTranslation } from "react-i18next";
import { CloseBehavior } from "../../types/config";
import { Button } from "./ui/button";
import { Checkbox } from "./ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "./ui/dialog";
import { Label } from "./ui/label";
import { RadioGroup, RadioGroupItem } from "./ui/radio-group";

interface CloseBehaviorDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onConfirm: (behavior: Exclude<CloseBehavior, "ask">, remember: boolean) => Promise<void>;
}

export function CloseBehaviorDialog({
  open,
  onOpenChange,
  onConfirm,
}: CloseBehaviorDialogProps) {
  const { t } = useTranslation();
  const [behavior, setBehavior] = useState<Exclude<CloseBehavior, "ask">>(
    "minimize_to_tray",
  );
  const [remember, setRemember] = useState(false);
  const [isConfirming, setIsConfirming] = useState(false);

  useEffect(() => {
    if (open) {
      setBehavior("minimize_to_tray");
      setRemember(false);
      setIsConfirming(false);
    }
  }, [open]);

  const handleConfirm = async () => {
    setIsConfirming(true);
    try {
      await onConfirm(behavior, remember);
    } finally {
      setIsConfirming(false);
    }
  };

  const handleOpenChange = (nextOpen: boolean) => {
    if (!isConfirming) onOpenChange(nextOpen);
  };

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent className="border-white/10 bg-[#1E2128] text-white sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{t("close_dialog.title")}</DialogTitle>
          <DialogDescription className="text-white/50">
            {t("close_dialog.description")}
          </DialogDescription>
        </DialogHeader>

        <RadioGroup
          value={behavior}
          onValueChange={(value) =>
            setBehavior(value as Exclude<CloseBehavior, "ask">)
          }
          className="gap-3"
        >
          <Label
            htmlFor="close-minimize-to-tray"
            className="flex cursor-pointer items-start gap-3 rounded-lg border border-[#FCE100]/60 bg-[#FCE100]/5 p-4 has-[[data-state=unchecked]]:border-white/10 has-[[data-state=unchecked]]:bg-black/20"
          >
            <RadioGroupItem
              id="close-minimize-to-tray"
              value="minimize_to_tray"
              className="mt-0.5"
            />
            <PanelTopClose className="mt-0.5 size-5 text-[#FCE100]" />
            <span className="space-y-1">
              <span className="block font-medium">
                {t("close_dialog.minimize")}
              </span>
              <span className="block text-sm font-normal text-white/50">
                {t("close_dialog.minimize_desc")}
              </span>
            </span>
          </Label>
          <Label
            htmlFor="close-exit"
            className="flex cursor-pointer items-start gap-3 rounded-lg border border-[#FCE100]/60 bg-[#FCE100]/5 p-4 has-[[data-state=unchecked]]:border-white/10 has-[[data-state=unchecked]]:bg-black/20"
          >
            <RadioGroupItem id="close-exit" value="exit" className="mt-0.5" />
            <LogOut className="mt-0.5 size-5 text-[#FCE100]" />
            <span className="space-y-1">
              <span className="block font-medium">
                {t("close_dialog.exit")}
              </span>
              <span className="block text-sm font-normal text-white/50">
                {t("close_dialog.exit_desc")}
              </span>
            </span>
          </Label>
        </RadioGroup>

        <Label
          htmlFor="remember-close-behavior"
          className="flex cursor-pointer items-center gap-2 text-sm text-white/70"
        >
          <Checkbox
            id="remember-close-behavior"
            checked={remember}
            onCheckedChange={(checked) => setRemember(checked === true)}
          />
          {t("close_dialog.remember")}
        </Label>

        <DialogFooter>
          <Button
            variant="outline"
            className="border-white/10 bg-transparent text-white hover:bg-white/10 hover:text-white"
            onClick={() => onOpenChange(false)}
            disabled={isConfirming}
          >
            {t("close_dialog.cancel")}
          </Button>
          <Button onClick={handleConfirm} disabled={isConfirming}>
            {t("close_dialog.confirm")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
