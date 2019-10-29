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
                {
                    !this.state.expanded
                        ? <span style={{ cursor: "pointer" }} onClick={() => { this.setState({ expanded: true }) }}>&nbsp;<i><u>More...</u></i></span>
                        : (
                            <div>
                                <Dialog fullScreen={false} aria-labelledby="customized-dialog-title" open={this.state.expanded}>
                                    <DialogTitle id="customized-dialog-title"><b>{this.state.tour.name}</b> Details</DialogTitle>
                                    <DialogContent>
                                        <ul>
                                            <li><a href="/">All Tours</a></li>
                                            <li><ChooseTourVersion tour={this.state.tour} /></li>
                                        </ul>
                                    </DialogContent>
                                    <DialogActions>
                                        <Button
                                            color="primary" size='large' variant='outlined'
                                            onClick={() => {
                                                this.setState({ expanded: false })
                                            }
                                            }>OK</Button>
                                    </DialogActions>
                                </Dialog>
                            
                            </div>
                          )
                }
               

            </div>
            )

    }
}
