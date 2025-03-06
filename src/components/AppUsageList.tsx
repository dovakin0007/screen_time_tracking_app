import { IAppUsageInfo } from "../App"


function AppUsageInfo(props: IAppUsageInfo)  {
    return (
        <div>
          <h2>App Usage Info</h2>
          <p><strong>App Name:</strong> {props.appName}</p>
          <p><strong>Total Hours:</strong> {props.totalHours}</p>
          <p><strong>Idle Hours:</strong> {props.idleHours}</p>
          <p><strong>Active Percentage:</strong> {props.activePercentage}%</p>
        </div>
      );
}



export default AppUsageInfo