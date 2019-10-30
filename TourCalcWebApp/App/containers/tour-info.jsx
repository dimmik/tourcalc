import React from 'react';
import ReactDOM from 'react-dom';
import AppState from './appstate.jsx'
import ChooseTourVersion from './tour-versions.jsx'

import { BrowserRouter as Router, Route, Switch, Redirect, withRouter } from 'react-router-dom';

import Dialog from '@material-ui/core/Dialog';
import DialogTitle from '@material-ui/core/DialogTitle';
import DialogContent from '@material-ui/core/DialogContent';
import DialogActions from '@material-ui/core/DialogActions';

import InputLabel from '@material-ui/core/InputLabel';
import Input from '@material-ui/core/Input';
import MenuItem from '@material-ui/core/MenuItem';
import FormHelperText from '@material-ui/core/FormHelperText';
import FormControl from '@material-ui/core/FormControl';
import Select from '@material-ui/core/Select';
import TextField from '@material-ui/core/TextField';
import Chip from '@material-ui/core/Chip';
import Checkbox from '@material-ui/core/Checkbox';
import ListItemText from '@material-ui/core/ListItemText';
import FormControlLabel from '@material-ui/core/FormControlLabel';
import FormGroup from '@material-ui/core/FormGroup';
import Button from '@material-ui/core/Button';
import ButtonGroup from '@material-ui/core/ButtonGroup';
import Grid from '@material-ui/core/Grid';



export default class TourInfo extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            tour: props.tour,
            updateTime: props.updateTime,
            expanded: false
        }
    }
    componentWillReceiveProps(props) {
        this.setState({ tour: props.tour, updateTime: props.updateTime });
    }
    componentDidMount() {
    }
    render() {

        return (
            <div style={{ fontSize: 'small' }}>
                <Grid container spacing={1} alignContent="center" alignItems="center">
                    <Grid item>
                {this.state.tour.isVersion ? <b style={{color: "red"}}>(V)</b> : ''}
                {this.state.tour.name} [
                                {
                    this.state.tour.persons.filter(p => (p.receivedInCents - p.spentInCents) >= 0).length > 0
                        ? ((1 - this.state.tour.persons.filter(p => (p.receivedInCents - p.spentInCents) > 0).length * 1.0 /
                            this.state.tour.persons.filter(p => (p.receivedInCents - p.spentInCents) >= 0).length) * 100)
                            .toFixed(0) : 0
                }%&nbsp;

                        {
                    (this.state.updateTime.getHours() + "").padStart(2, '0') + ':' +
                    (this.state.updateTime.getMinutes() + "").padStart(2, '0') + ':' +
                    (this.state.updateTime.getSeconds() + "").padStart(2, '0')
                        }]
                </Grid>
                    <Grid item>
                {
                    !this.state.expanded
                        ? <ButtonGroup variant="outlined" size="small" aria-label="small contained button group">
                                    <Button color="primary" onClick={() => { this.setState({ expanded: true }) }}>More</Button>
                                    {this.state.tour.isVersion
                                        ? <Button color="secondary" onClick={() => { window.location = '/tour/' + this.props.tour.versionFor_Id + '/persons' }}>Back to tour</Button>
                                        : <Button color="secondary" onClick={() => { window.location = '/' }}>Tour List</Button>}
                        </ButtonGroup>
                        : (
                            <div>
                                <Dialog fullScreen={true} aria-labelledby="customized-dialog-title" open={this.state.expanded}>
                                    <DialogTitle id="customized-dialog-title">Tour<b>'{this.state.tour.name}'</b></DialogTitle>
                                            <DialogContent>
                                                <Grid container direction="column" spacing={2}>
                                                    <Grid item>
                                                        <ChooseTourVersion tour={this.state.tour} />
                                                    </Grid>
                                                </Grid>
                                    </DialogContent>
                                    <DialogActions>
                                        <Button
                                            color="secondary" size='large' variant='outlined'
                                            onClick={() => {
                                                this.setState({ expanded: false })
                                            }
                                            }>Close</Button>
                                    </DialogActions>
                                </Dialog>
                            
                            </div>
                          )
                        }
                    </Grid>
                </Grid>
            </div>
            )

    }
}
