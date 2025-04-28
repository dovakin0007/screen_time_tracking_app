import React from "react";
import { Box, Grid, TextField, Typography } from "@mui/material";

interface TimeLimitInputProps {
  hours: number;
  minutes: number;
  onHoursChange: (value: number) => void;
  onMinutesChange: (value: number) => void;
  error?: string;
}

export const TimeLimitInput: React.FC<TimeLimitInputProps> = ({
  hours,
  minutes,
  onHoursChange,
  onMinutesChange,
  error,
}) => {
  const clamp = (value: number, min: number, max: number) =>
    Math.min(Math.max(value, min), max);

  const handleHoursChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const value = parseInt(e.target.value, 10);
    onHoursChange(isNaN(value) ? 0 : clamp(value, 0, 24));
  };

  const handleMinutesChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const value = parseInt(e.target.value, 10);
    onMinutesChange(isNaN(value) ? 0 : clamp(value, 0, 59));
  };

  return (
    <Box
      sx={{
        p: 2,
        border: "1px solid #e0e0e0",
        borderRadius: "12px",
        bgcolor: "background.paper",
        marginBottom: 3,
      }}
    >
      <Typography variant="subtitle2" gutterBottom fontWeight="medium">
        Daily Time Limit
      </Typography>
      <Grid container spacing={2}>
        <Grid size={{ xs: 6 }}>
          <TextField
            label="Hours"
            type="number"
            value={hours}
            onChange={handleHoursChange}
            slotProps={{
              htmlInput: {
                type: "number",
                min: 0,
                max: 24,
              },
            }}
            fullWidth
            size="small"
            variant="outlined"
          />
        </Grid>
        <Grid size={{ xs: 6 }}>
          <TextField
            label="Minutes"
            type="number"
            value={minutes}
            onChange={handleMinutesChange}
            slotProps={{
              htmlInput: {
                type: "number",
                min: 0,
                max: 59,
              },
            }}
            fullWidth
            size="small"
            variant="outlined"
          />
        </Grid>
      </Grid>
      {error && (
        <Typography
          variant="caption"
          color="error"
          sx={{ marginTop: 1, display: "block" }}
        >
          {error}
        </Typography>
      )}
    </Box>
  );
};
