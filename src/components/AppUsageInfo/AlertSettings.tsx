import React from "react";
import {
  Box,
  Checkbox,
  FormControlLabel,
  Slider,
  Typography,
} from "@mui/material";

interface AlertSettingsProps {
  shouldAlert: boolean;
  shouldClose: boolean;
  alertBeforeClose: boolean;
  alertDuration: number;
  onAlertChange: (checked: boolean) => void;
  onCloseChange: (checked: boolean) => void;
  onAlertBeforeCloseChange: (checked: boolean) => void;
  onAlertDurationChange: (value: number) => void;
}

export const AlertSettings: React.FC<AlertSettingsProps> = ({
  shouldAlert,
  shouldClose,
  alertBeforeClose,
  alertDuration,
  onAlertChange,
  onCloseChange,
  onAlertBeforeCloseChange,
  onAlertDurationChange,
}) => {
  const handleAlertDurationSliderChange = (
    _event: Event,
    newValue: number | number[],
  ) => {
    onAlertDurationChange(newValue as number);
  };

  return (
    <Box sx={{ display: "flex", flexDirection: "column", gap: 1 }}>
      <FormControlLabel
        control={
          <Checkbox
            checked={shouldAlert}
            onChange={(e) => onAlertChange(e.target.checked)}
            disabled={shouldClose}
            size="small"
          />
        }
        label={
          <Typography variant="body2">
            Alert when limit is reached
          </Typography>
        }
      />

      {shouldAlert && (
        <Box sx={{ ml: 3, mb: 2 }}>
          <Typography
            variant="caption"
            color="text.secondary"
            display="block"
            gutterBottom
          >
            Alert Duration (seconds)
          </Typography>
          <Box sx={{ display: "flex", alignItems: "center", gap: 2 }}>
            <Slider
              value={alertDuration}
              onChange={handleAlertDurationSliderChange}
              min={10}
              max={600}
              step={10}
              valueLabelDisplay="auto"
              aria-labelledby="alert-duration-slider"
              sx={{ width: "calc(100% - 40px)" }}
            />
            <Typography
              variant="body2"
              sx={{ minWidth: 40, textAlign: "right" }}
            >
              {alertDuration}s
            </Typography>
          </Box>
        </Box>
      )}

      <FormControlLabel
        control={
          <Checkbox
            checked={shouldClose}
            onChange={(e) => onCloseChange(e.target.checked)}
            disabled={shouldAlert}
            size="small"
          />
        }
        label={
          <Typography variant="body2">
            Close app when limit is reached
          </Typography>
        }
      />

      {shouldClose && (
        <Box sx={{ ml: 3 }}>
          <FormControlLabel
            control={
              <Checkbox
                checked={alertBeforeClose}
                onChange={(e) => onAlertBeforeCloseChange(e.target.checked)}
                size="small"
              />
            }
            label={
              <Typography variant="body2">Alert before closing</Typography>
            }
          />
        </Box>
      )}
    </Box>
  );
};
