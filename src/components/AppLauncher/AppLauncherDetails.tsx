import React from "react";
import { Box, Typography } from "@mui/material";
import { AppLaunchButton } from "./AppLauncherButton";
import { AppLauncherDetailsProps } from "./types";

export const AppLauncherDetails: React.FC<AppLauncherDetailsProps> = ({
  description,
  link,
  launchCount,
  lastLaunched,
  onLaunch,
}) => (
  <Box sx={{ pl: 9, pr: 2, pb: 2 }}>
    {description && (
      <Typography variant="body2" sx={{ mb: 1 }} color="text.primary">
        {description}
      </Typography>
    )}
    <Typography
      variant="body2"
      sx={{ mb: 1, wordBreak: "break-all" }}
      color="text.secondary"
    >
      {link}
    </Typography>
    {typeof launchCount === "number" && (
      <Typography variant="caption" display="block" color="text.secondary">
        Launches: {launchCount}
      </Typography>
    )}
    {lastLaunched && (
      <Typography variant="caption" display="block" color="text.secondary">
        Last Launched: {new Date(lastLaunched).toLocaleString()}
      </Typography>
    )}
    <AppLaunchButton onClick={onLaunch} />
  </Box>
);
