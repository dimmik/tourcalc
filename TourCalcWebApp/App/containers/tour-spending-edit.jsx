import React from 'react';
import ReactDOM from 'react-dom';
import AppState from './appstate.jsx'

import Dialog from '@material-ui/core/Dialog';
import DialogTitle from '@material-ui/core/DialogTitle';
import DialogContent from '@material-ui/core/DialogContent';
import DialogActions from '@material-ui/core/DialogActions';


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
                <button onClick={() => this.setState({ dialogOpen: true })}>
                    {this.props.buttonText}
                </button>
                <Dialog aria-labelledby="customized-dialog-title" open={this.state.dialogOpen}>
                    <DialogTitle id="customized-dialog-title">Add Spending</DialogTitle>
                    <DialogContent>
                        <form onSubmit={(event) => {}}>
                            <p>description:</p>
                            <input
                                type='text'
                                onChange={(e) => this.spending.description = event.target.value}
                                defaultValue={this.spending.description}
                            />
                            <p>amount:</p>
                            <input
                                type='number'
                                onChange={(e) => this.spending.amountInCents = event.target.value}
                                defaultValue={this.spending.amountInCents}
                            /><br/><br/>
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