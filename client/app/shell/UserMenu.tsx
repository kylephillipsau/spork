import { KeyRound, LogOut, Monitor, Moon, Sun, UserRound } from "lucide-react";

import {
  Avatar,
  Menu,
  MenuItem,
  MenuLabel,
  MenuRadioGroup,
  MenuSeparator,
  useTheme,
  type ThemePreference,
} from "@ui/index";
import { api } from "@domain/api";
import { href } from "@app/routing/location";
import { useSession } from "@app/session/SessionContext";

import s from "./header.module.css";

/** The account, the theme, and signing out. */
export function UserMenu() {
  const session = useSession();
  const { preference, setPreference } = useTheme();
  if (session.kind !== "signed-in") return null;
  const who = session.who;

  return (
    <Menu
      trigger={
        <button type="button" className={s.user} aria-label={`Account: ${who.display_name}`}>
          <Avatar name={who.display_name} size={28} />
          <span className={s.userName}>{who.display_name}</span>
        </button>
      }
    >
      <div className={s.userCard}>
        <span className={s.userCardName}>{who.display_name}</span>
        <span className={s.userCardOrg}>{who.tenant_name}</span>
      </div>
      <MenuSeparator />
      <MenuItem icon={<UserRound />} href={href("/account")}>
        Account and password
      </MenuItem>
      <MenuItem icon={<KeyRound />} href={href("/keys")}>
        Passkeys
      </MenuItem>
      <MenuSeparator />
      <MenuLabel>Theme</MenuLabel>
      <MenuRadioGroup
        value={preference}
        onValueChange={(v) => setPreference(v as ThemePreference)}
        options={[
          { value: "light", label: "Light", icon: <Sun /> },
          { value: "dark", label: "Dark", icon: <Moon /> },
          { value: "system", label: "System", icon: <Monitor /> },
        ]}
      />
      <MenuSeparator />
      <MenuItem
        icon={<LogOut />}
        danger
        onSelect={() => {
          void api.signOff().finally(() => window.location.assign(href("/sign-in")));
        }}
      >
        Sign out
      </MenuItem>
    </Menu>
  );
}
