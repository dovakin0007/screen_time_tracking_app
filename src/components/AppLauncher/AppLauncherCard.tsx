import React, { useState } from "react";
import { Collapse } from "@mui/material";
import { AppLauncherHeader } from "./AppLauncherHeader";
import { AppLauncherDetails } from "./AppLauncherDetails";
import { AppLauncherCardProps } from "./types";

export const AppLauncherCard: React.FC<AppLauncherCardProps> = ({
  app,
  expanded,
  onToggle,
  onLaunch,
  launchCount,
  lastLaunched,
}) => {
  const [iconError, setIconError] = useState(false);
  const appName = app.link.split("\\").pop()?.replace(/\.lnk$/i, "") || "App";

  return (
    <>
      <AppLauncherHeader
        appName={appName}
        link={app.link}
        icon={!iconError ? app.icon_base64_image : undefined}
        expanded={expanded}
        onToggle={onToggle}
        onIconError={() => setIconError(true)}
      />

      <Collapse in={expanded} timeout="auto" unmountOnExit>
        <AppLauncherDetails
          description={app.description}
          link={app.link}
          launchCount={launchCount}
          lastLaunched={lastLaunched}
          onLaunch={() => onLaunch(app.link)}
        />
      </Collapse>
    </>
  );
};

export default AppLauncherCard;
