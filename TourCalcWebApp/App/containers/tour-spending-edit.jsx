import React from 'react';
import ReactDOM from 'react-dom';
import AppState from './appstate.jsx'

import Dialog from '@material-ui/core/Dialog';
import DialogTitle from '@material-ui/core/DialogTitle';
import DialogContent from '@material-ui/core/DialogContent';
import DialogActions from '@material-ui/core/DialogActions';

import InputLabel from '@material-ui/core/InputLabel';
import MenuItem from '@material-ui/core/MenuItem';
import FormHelperText from '@material-ui/core/FormHelperText';
import FormControl from '@material-ui/core/FormControl';
import Select from '@material-ui/core/Select';
import TextField from '@material-ui/core/TextField';

export default class SpendingsForm extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            dialogOpen: props.open
        }
        this.tour = props.tour

        if (props.spending != null) this.spending = props.spending
        else this.spending.fromGuid = this.tour.persons.length > 0 ? this.tour.persons[0].guid : ""
    }
    spending = {
        description: "",
        amountInCents: 0,
        fromGuid: null,
        toGuid: [],
        toAll: false,
        guid: ""
    }

    render() {
        return (
            <span>
                <span style={{ cursor: "pointer" }} onClick={() => this.setState({ dialogOpen: true })}>
                    {this.props.buttonText}
                </span>
                <Dialog fullWidth={true} aria-labelledby="customized-dialog-title" open={this.state.dialogOpen}>
                    <DialogTitle id="customized-dialog-title">{this.props.mode == 'edit' ? 'Edit' : 'Add'} Spending</DialogTitle>
                    <DialogContent>
                        <form onSubmit={(event) => { }}>
                            <FormControl>
                                <TextField
                                    id="description"
                                    required
                                    label="Description"
                                    defaultValue={this.spending.description}
                                    onChange={(e) => this.spending.description = event.target.value}
                                    margin="normal"
                                />
                                <TextField
                                    id="amount"
                                    required
                                    label="Amount"
                                    type="number"
                                    defaultValue={this.spending.amountInCents}
                                    onChange={(e) => this.spending.amountInCents = event.target.value}
                                    margin="normal"
                                />
                                <Select
                                    value={this.spending.fromGuid}
                                    onChange={() => { this.spending.fromGuid = event.target.value }}
                                >
                                    {
                                        this.tour.persons.map(p => (<MenuItem value={p.guid} key={p.guid}>{p.name}</MenuItem>))
                                    }
                                </Select>
                            </FormControl>
                            <label>from:
                             <select
                                    defaultValue={this.spending.fromGuid}
                                    onChange={() => { this.spending.fromGuid = event.target.value }}
                                >
                                    {
                                        this.tour.persons.map(p => (<option value={p.guid} key={p.guid}>{p.name}</option>))
                                    }
                                </select></label>
                            <br /><br />
                            <label>to:
                             <select
                                    defaultValue={this.spending.toGuid}
                                    multiple={true}
                                    onChange={(e) => { this.spending.toGuid = Array.from(e.target.options).filter(o => o.selected).map(o => o.value) }}
                            >
                                {
                                        this.tour.persons.map(p => (<option value={p.guid} key={p.guid}>{p.name}</option>))
                                    }
                                </select></label>
                            <br />toAll: <input type="checkbox" defaultChecked={this.spending.toAll}
                                onChange={(e) => { this.spending.toAll = e.target.checked }}
                            />ToAll
                            <br />
                        </form>
                    </DialogContent>
                    <DialogActions>
                        <button color="primary" onClick={() => {
                            //alert('sp: ' + JSON.stringify(this.spending, null, 2))
                            (this.props.mode === "add"
                                ? AppState.addSpending(this.props.app, this.tour.id, this.spending)
                                : AppState.editSpending(this.props.app, this.tour.id, this.spending)
                            )
                                .then(this.setState({ dialogOpen: false }))
                                .then(() => { AppState.loadTour(this.props.app, this.tour.id) })
                              
                              
                        }}>{this.props.actionButtonText}</button>
                        <button onClick={() => { this.setState({ dialogOpen: false }) }}>Cancel</button>
                    </DialogActions>
                </Dialog>
            </span>
        )
    }
}