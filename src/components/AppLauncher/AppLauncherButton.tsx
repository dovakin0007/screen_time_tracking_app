import React from "react";
import { Button } from "@mui/material";
import { Launch } from "@mui/icons-material";

interface AppLaunchButtonProps {
  onClick: () => void;
}

export const AppLaunchButton: React.FC<AppLaunchButtonProps> = (
  { onClick },
) => (
  <Button
    variant="contained"
    size="small"
    startIcon={<Launch />}
    onClick={onClick}
    sx={{ mt: 1 }}
  >
    Launch
  </Button>
);
