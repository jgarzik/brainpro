import { useState } from "react";
import { Modal } from "@/components/ui/Modal";
import { Input } from "@/components/ui/Input";
import { Button } from "@/components/ui/Button";
import { useConnectionStore } from "@/store/connectionStore";
import { useUIStore } from "@/store/uiStore";

export function PasswordDialog() {
  const open = useUIStore((s) => s.passwordDialogOpen);
  const setOpen = useUIStore((s) => s.setPasswordDialogOpen);
  const { connect, error } = useConnectionStore();

  const [password, setPassword] = useState("");
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    try {
      await connect(password);
      setOpen(false);
      setPassword("");
    } catch {
      // Error handled in store
    } finally {
      setLoading(false);
    }
  };

  const handleSkip = async () => {
    setLoading(true);
    try {
      await connect();
      setOpen(false);
    } catch {
      // Error handled in store
    } finally {
      setLoading(false);
    }
  };

  return (
    <Modal
      open={open}
      onClose={() => setOpen(false)}
      title="Connect to Gateway"
      size="sm"
    >
      <form onSubmit={handleSubmit}>
        <p className="mb-4 text-sm text-gray-600 dark:text-gray-400">
          Enter the gateway password to connect. Leave empty if no password is
          configured.
        </p>

        <Input
          type="password"
          label="Password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          placeholder="Optional"
          error={error ?? undefined}
          autoFocus
        />

        <div className="mt-6 flex justify-end gap-3">
          <Button
            type="button"
            variant="ghost"
            onClick={handleSkip}
            disabled={loading}
          >
            Skip
          </Button>
          <Button type="submit" loading={loading}>
            Connect
          </Button>
        </div>
      </form>
    </Modal>
  );
}
