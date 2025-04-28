import React from "react";
import { Grid, Paper, Typography } from "@mui/material";

interface UsageStatsProps {
  totalHours: number;
  activePercentage: number;
  idleHours: number;
}

export const UsageStats: React.FC<UsageStatsProps> = ({
  totalHours,
  activePercentage,
  idleHours,
}) => {
  return (
    <Grid container spacing={2} sx={{ marginBottom: 3 }}>
      <Grid size={{ xs: 12, sm: 4 }}>
        <Paper
          elevation={0}
          sx={{ p: 2, border: "1px solid #e0e0e0", borderRadius: "8px" }}
        >
          <Typography
            variant="caption"
            color="text.secondary"
            display="block"
            gutterBottom
          >
            Total Usage
          </Typography>
          <Typography variant="subtitle1" fontWeight="medium">
            {totalHours}h
          </Typography>
        </Paper>
      </Grid>
      <Grid size={{ xs: 12, sm: 4 }}>
        <Paper
          elevation={0}
          sx={{ p: 2, border: "1px solid #e0e0e0", borderRadius: "8px" }}
        >
          <Typography
            variant="caption"
            color="text.secondary"
            display="block"
            gutterBottom
          >
            Active Usage
          </Typography>
          <Typography variant="subtitle1" fontWeight="medium">
            {((totalHours * activePercentage) / 100).toFixed(1)}h
          </Typography>
        </Paper>
      </Grid>
      <Grid size={{ xs: 12, sm: 4 }}>
        <Paper
          elevation={0}
          sx={{ p: 2, border: "1px solid #e0e0e0", borderRadius: "8px" }}
        >
          <Typography
            variant="caption"
            color="text.secondary"
            display="block"
            gutterBottom
          >
            Idle Time
          </Typography>
          <Typography variant="subtitle1" fontWeight="medium">
            {idleHours}h
          </Typography>
        </Paper>
      </Grid>
    </Grid>
  );
};
