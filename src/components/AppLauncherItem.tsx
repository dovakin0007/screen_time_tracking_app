import React, { useState } from "react";
import {
  Avatar,
  Box,
  Button,
  Collapse,
  IconButton,
  ListItem,
  ListItemAvatar,
  ListItemText,
  Typography,
} from "@mui/material";
import { ExpandLess, ExpandMore, Launch } from "@mui/icons-material";

interface ShellLinkInfo {
  link: string;
  target_path: string;
  arguments?: string;
  icon_base64_image?: string;
  working_directory?: string;
  description?: string;
}

interface Props {
  app: ShellLinkInfo;
  expanded: boolean;
  onToggle: () => void;
  onLaunch: (link: string) => void;
  launchCount?: number;
  lastLaunched?: string | null;
}

const AppLauncherCard: React.FC<Props> = ({
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
      <ListItem
        component="button"
        onClick={onToggle}
        secondaryAction={
          <IconButton edge="end">
            {expanded ? <ExpandLess /> : <ExpandMore />}
          </IconButton>
        }
        sx={{
          "&:hover": { backgroundColor: "#f5f5f5" },
        }}
      >
        <ListItemAvatar>
          <Avatar
            src={!iconError && app.icon_base64_image
              ? `data:image/png;base64,${app.icon_base64_image}`
              : undefined}
            alt={appName}
            onError={() => setIconError(true)}
          >
            {appName.charAt(0)}
          </Avatar>
        </ListItemAvatar>
        <ListItemText
          primary={appName}
          secondary={!expanded ? app.link : undefined}
        />
      </ListItem>

      <Collapse in={expanded} timeout="auto" unmountOnExit>
        <Box sx={{ pl: 9, pr: 2, pb: 2 }}>
          {app.description && (
            <Typography variant="body2" sx={{ mb: 1 }} color="text.primary">
              {app.description}
            </Typography>
          )}
          <Typography
            variant="body2"
            sx={{ mb: 1, wordBreak: "break-all" }}
            color="text.secondary"
          >
            {app.link}
          </Typography>
          {typeof launchCount === "number" && (
            <Typography
              variant="caption"
              display="block"
              color="text.secondary"
            >
              Launches: {launchCount}
            </Typography>
          )}
          {lastLaunched && (
            <Typography
              variant="caption"
              display="block"
              color="text.secondary"
            >
              Last Launched: {new Date(lastLaunched).toLocaleString()}
            </Typography>
          )}
          <Button
            variant="contained"
            size="small"
            startIcon={<Launch />}
            onClick={() => onLaunch(app.link)}
            sx={{ mt: 1 }}
          >
            Launch
          </Button>
        </Box>
      </Collapse>
    </>
  );
};

export default AppLauncherCard;
