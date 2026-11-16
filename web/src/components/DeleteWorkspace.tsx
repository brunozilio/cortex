import { Icon, type IconName } from "@agenteschat/ui";
import type { ReactNode } from "react";
import "./deletion.css";

export function DeletionLine({ icon, children }: { icon: IconName; children: ReactNode }) {
  return (
    <li className="ac-iconline">
      <Icon name={icon} size={16} />
      <span>{children}</span>
    </li>
  );
}
