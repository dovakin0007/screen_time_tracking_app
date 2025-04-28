import React from "react";
import {
  Avatar,
  IconButton,
  ListItem,
  ListItemAvatar,
  ListItemText,
  Typography,
} from "@mui/material";
import { ExpandLess, ExpandMore } from "@mui/icons-material";
import { AppLauncherHeaderProps } from "./types";

export const AppLauncherHeader: React.FC<AppLauncherHeaderProps> = ({
  appName,
  link,
  icon,
  expanded,
  onToggle,
  onIconError,
}) => (
  <ListItem
    component="button"
    onClick={onToggle}
    secondaryAction={
      <IconButton edge="end" aria-label={expanded ? "Collapse" : "Expand"}>
        {expanded ? <ExpandLess /> : <ExpandMore />}
      </IconButton>
    }
    sx={{
      "&:hover": { backgroundColor: "action.hover" },
      cursor: "pointer",
      transition: "background-color 0.2s ease",
    }}
  >
    <ListItemAvatar>
      <Avatar 
        src={`data:image/png;base64,${icon}`} 
        alt={appName}
        onError={onIconError}
        sx={{
          bgcolor: icon ? 'transparent' : 'primary.main',
          color: icon ? undefined : 'primary.contrastText'
        }}
      >
        {appName.charAt(0).toUpperCase()}
      </Avatar>
    </ListItemAvatar>
    <ListItemText
      primary={
        <Typography variant="body1" fontWeight="medium">
          {appName}
        </Typography>
      }
      secondary={
        !expanded && (
          <Typography variant="body2" color="text.secondary" noWrap>
            {link}
          </Typography>
        )
      }
      sx={{
        overflow: 'hidden',
        textOverflow: 'ellipsis'
      }}
    />
  </ListItem>
);