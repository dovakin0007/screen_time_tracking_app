export interface ShellLinkInfo {
  link: string;
  target_path: string;
  arguments?: string;
  icon_base64_image?: string;
  working_directory?: string;
  description?: string;
}

export interface AppLauncherCardProps {
  app: ShellLinkInfo;
  expanded: boolean;
  onToggle: () => void;
  onLaunch: (link: string) => void;
  launchCount?: number;
  lastLaunched?: string | null;
}

export interface AppLauncherHeaderProps {
  appName: string;
  link: string;
  icon: string | undefined;
  expanded: boolean;
  onToggle: () => void;
  onIconError: () => void;
}

export interface AppLauncherDetailsProps {
  description?: string;
  link: string;
  launchCount?: number;
  lastLaunched?: string | null;
  onLaunch: () => void;
}
